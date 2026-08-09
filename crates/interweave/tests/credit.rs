//! 23.28: gates that cannot be scored past, the four Shapley axioms, and undischarged confounders.

use bioprism_interweave::credit::{
    difference_reward, evaluate, shapley, shapley_all, CardField, CoalitionValues, Confounder,
    CreditError, CreditEstimate, Decision, Discharge, Evaluation, EvolutionCard, GateResult, Method,
    PolicySlot, Rational, RewardDimension, SafetyGate, MAX_EXACT_PARTICIPANTS,
};
use std::collections::{BTreeMap, BTreeSet};

fn rewards() -> BTreeMap<RewardDimension, i64> {
    BTreeMap::from([
        (RewardDimension::EndTaskUtility, 90),
        (RewardDimension::CostAndLatency, -12),
    ])
}

fn held(name: &str) -> GateResult {
    GateResult {
        gate: SafetyGate::new(name),
        held: true,
        detail: String::new(),
    }
}

fn failed(name: &str, detail: &str) -> GateResult {
    GateResult {
        gate: SafetyGate::new(name),
        held: false,
        detail: detail.into(),
    }
}

#[test]
fn a_run_with_a_failed_gate_yields_no_reward_vector_at_all() {
    let evaluation = evaluate(
        &[held("no-ambient-credentials"), failed("no-e4-without-human", "published before approval")],
        rewards(),
    );
    assert!(evaluation.blocked());
    assert_eq!(evaluation.rewards(), None);
}

#[test]
fn an_excellent_run_that_fails_one_gate_scores_exactly_as_low_as_a_terrible_one() {
    let excellent = evaluate(&[failed("gate", "why")], rewards());
    let terrible = evaluate(
        &[failed("gate", "why")],
        BTreeMap::from([(RewardDimension::EndTaskUtility, -1000)]),
    );
    assert_eq!(excellent, terrible);
}

#[test]
fn a_run_passing_every_gate_carries_its_reward_vector() {
    let evaluation = evaluate(&[held("a"), held("b")], rewards());
    assert_eq!(evaluation.rewards(), Some(&rewards()));
}

#[test]
fn the_first_failing_gate_is_the_reported_one_so_evaluation_is_deterministic() {
    let evaluation = evaluate(
        &[failed("first", "one"), failed("second", "two")],
        rewards(),
    );
    match evaluation {
        Evaluation::Blocked { gate, detail } => {
            assert_eq!(gate, SafetyGate::new("first"));
            assert_eq!(detail, "one");
        }
        other => panic!("expected blocked, got {other:?}"),
    }
}

/// A three-player game where `c` contributes nothing to any coalition.
fn game_with_dummy() -> CoalitionValues {
    let values = [
        (vec![], 0),
        (vec!["a"], 10),
        (vec!["b"], 20),
        (vec!["c"], 0),
        (vec!["a", "b"], 40),
        (vec!["a", "c"], 10),
        (vec!["b", "c"], 20),
        (vec!["a", "b", "c"], 40),
    ];
    CoalitionValues::new(
        ["a", "b", "c"],
        values.into_iter().map(|(members, value)| {
            (
                members.into_iter().map(str::to_string).collect::<BTreeSet<_>>(),
                value,
            )
        }),
    )
    .expect("every coalition is valued")
}

/// A symmetric two-player game.
fn symmetric_game() -> CoalitionValues {
    let values = [
        (vec![], 0),
        (vec!["x"], 5),
        (vec!["y"], 5),
        (vec!["x", "y"], 14),
    ];
    CoalitionValues::new(
        ["x", "y"],
        values.into_iter().map(|(members, value)| {
            (
                members.into_iter().map(str::to_string).collect::<BTreeSet<_>>(),
                value,
            )
        }),
    )
    .expect("every coalition is valued")
}

#[test]
fn a_game_missing_one_coalition_cannot_be_built_and_the_gap_is_not_a_zero() {
    let error = CoalitionValues::new(
        ["a", "b"],
        [
            (BTreeSet::new(), 0),
            (BTreeSet::from(["a".to_string()]), 1),
            (BTreeSet::from(["a".to_string(), "b".to_string()]), 3),
        ],
    )
    .unwrap_err();
    assert_eq!(
        error,
        CreditError::UnvaluedCoalition {
            coalition: BTreeSet::from(["b".to_string()])
        }
    );
}

#[test]
fn shapley_values_sum_to_the_grand_coalition_minus_the_empty_one() {
    let game = game_with_dummy();
    let values = shapley_all(&game).expect("computable");
    let total = values
        .values()
        .copied()
        .fold(Rational::zero(), |acc, value| acc + value);
    let expected = game.grand().expect("valued") - game.empty().expect("valued");
    assert_eq!(total.denominator, 1);
    assert_eq!(total.numerator, i128::from(expected));
}

#[test]
fn a_dummy_player_contributing_nothing_to_any_coalition_receives_exactly_zero() {
    let value = shapley(&game_with_dummy(), "c").expect("computable");
    assert!(value.is_zero());
}

#[test]
fn two_symmetric_participants_receive_identical_shapley_values() {
    let game = symmetric_game();
    assert_eq!(
        shapley(&game, "x").expect("computable"),
        shapley(&game, "y").expect("computable")
    );
}

#[test]
fn shapley_is_additive_across_two_games_over_the_same_participants() {
    let left = symmetric_game();
    let right = symmetric_game();
    let combined = left.plus(&right).expect("same participants");
    let separate = shapley(&left, "x").expect("computable") + shapley(&right, "x").expect("ok");
    assert_eq!(shapley(&combined, "x").expect("computable"), separate);
}

#[test]
fn shapley_is_exact_and_reduced_rather_than_rounded() {
    let value = shapley(&symmetric_game(), "x").expect("computable");
    assert_eq!(value, Rational::new(7, 1).expect("nonzero denominator"));
}

#[test]
fn an_odd_shapley_value_keeps_its_fraction_instead_of_truncating() {
    let values = [
        (vec![], 0),
        (vec!["a"], 0),
        (vec!["b"], 0),
        (vec!["a", "b"], 1),
    ];
    let game = CoalitionValues::new(
        ["a", "b"],
        values.into_iter().map(|(members, value)| {
            (
                members.into_iter().map(str::to_string).collect::<BTreeSet<_>>(),
                value,
            )
        }),
    )
    .expect("valued");
    assert_eq!(
        shapley(&game, "a").expect("computable"),
        Rational::new(1, 2).expect("nonzero denominator")
    );
}

#[test]
fn the_difference_reward_is_the_grand_coalition_minus_the_team_without_the_participant() {
    let game = game_with_dummy();
    assert_eq!(difference_reward(&game, "a").expect("known"), 20);
    assert_eq!(difference_reward(&game, "c").expect("known"), 0);
}

#[test]
fn a_participant_outside_the_grand_coalition_is_an_error_rather_than_a_zero() {
    let game = game_with_dummy();
    assert_eq!(
        difference_reward(&game, "d"),
        Err(CreditError::UnknownParticipant {
            participant: "d".into()
        })
    );
    assert!(shapley(&game, "d").is_err());
}

#[test]
fn shapley_refuses_above_its_exact_bound_rather_than_switching_to_sampling() {
    let names: Vec<String> = (0..=MAX_EXACT_PARTICIPANTS)
        .map(|index| format!("p{index}"))
        .collect();
    let error = CoalitionValues::new(names, []).unwrap_err();
    assert_eq!(
        error,
        CreditError::TooManyParticipants {
            count: MAX_EXACT_PARTICIPANTS + 1,
            limit: MAX_EXACT_PARTICIPANTS,
        }
    );
}

#[test]
fn adding_two_games_over_different_participants_is_refused() {
    assert_eq!(
        symmetric_game().plus(&game_with_dummy()),
        Err(CreditError::ParticipantMismatch)
    );
}

#[test]
fn a_new_credit_estimate_starts_with_every_confounder_undischarged() {
    let estimate = CreditEstimate::new(
        "skeptic",
        PolicySlot::ChallengeAndVerification,
        Method::DifferenceRewards,
        Rational::integer(12),
    );
    assert_eq!(estimate.outstanding().len(), Confounder::ALL.len());
    assert!(!estimate.reportable());
}

#[test]
fn one_undischarged_confounder_is_enough_to_make_an_estimate_unreportable() {
    let estimate = Confounder::ALL
        .into_iter()
        .filter(|c| *c != Confounder::CorrelatedParticipants)
        .fold(
            CreditEstimate::new(
                "skeptic",
                PolicySlot::Aggregation,
                Method::ShapleyStyleApproximation,
                Rational::integer(3),
            ),
            |estimate, confounder| estimate.controlling(confounder, "held fixed across arms"),
        );
    assert_eq!(
        estimate.outstanding(),
        BTreeSet::from([Confounder::CorrelatedParticipants])
    );
    assert!(!estimate.reportable());
}

#[test]
fn an_estimate_with_every_confounder_discharged_is_reportable_and_says_how() {
    let estimate = Confounder::ALL.into_iter().fold(
        CreditEstimate::new(
            "patcher",
            PolicySlot::RoleMatching,
            Method::DifferenceRewards,
            Rational::integer(7),
        ),
        |estimate, confounder| estimate.controlling(confounder, "stratified by parent lineage"),
    );
    assert!(estimate.reportable());
    assert!(matches!(
        estimate.discharge(Confounder::HiddenSharedEvidence),
        Discharge::Controlled { .. }
    ));
}

#[test]
fn only_two_of_the_six_credit_methods_are_computed_in_this_crate() {
    let computed: Vec<Method> = Method::ALL.into_iter().filter(|m| m.computed_here()).collect();
    assert_eq!(
        computed,
        vec![Method::DifferenceRewards, Method::ShapleyStyleApproximation]
    );
}

#[test]
fn an_evolution_card_with_a_blank_required_field_is_incomplete() {
    let card = CardField::ALL.into_iter().fold(EvolutionCard::new(), |card, field| {
        if field == CardField::RollbackHash {
            card.stating(field, "   ")
        } else {
            card.stating(field, "stated")
        }
    });
    assert_eq!(card.missing(), BTreeSet::from([CardField::RollbackHash]));
    assert!(!card.complete());
}

#[test]
fn known_regressions_recorded_as_none_counts_as_stated_rather_than_missing() {
    let card = CardField::ALL.into_iter().fold(EvolutionCard::new(), |card, field| {
        if field == CardField::KnownRegressions {
            card.stating(field, "none observed on the holdout")
        } else {
            card.stating(field, "stated")
        }
    });
    assert!(card.complete());
}

#[test]
fn an_empty_evolution_card_owes_all_eight_fields() {
    assert_eq!(EvolutionCard::new().missing().len(), 8);
}

#[test]
fn the_thirteen_learnable_decisions_and_eight_policy_slots_are_distinct_vocabularies() {
    assert_eq!(Decision::ALL.len(), 13);
    assert_eq!(PolicySlot::ALL.len(), 8);
    assert_eq!(RewardDimension::ALL.len(), 10);
}
