//! Blueprint 43.50: value of information, bundles, and complementarity.

use bioprism_epistemic::decision::{Belief, DecisionProblem};
use bioprism_epistemic::evidence::{Acquisition, Outcome};
use bioprism_epistemic::rng::SplitMix64;
use bioprism_epistemic::voi::{complementarity, joint_value, value_of_information};
use bioprism_epistemic::EpistemicError;

fn problem(actions: usize, models: usize, rng: &mut SplitMix64) -> DecisionProblem {
    DecisionProblem::new(
        (0..actions).map(|a| format!("a{a}")).collect(),
        (0..models).map(|m| format!("m{m}")).collect(),
        (0..actions * models).map(|_| rng.between(0.0, 1.0)).collect(),
    )
    .expect("well-formed")
}

#[test]
fn the_gross_value_of_information_is_never_negative_over_a_seeded_family() {
    let mut rng = SplitMix64::new(0x1234_5678_9ABC);
    for _ in 0..400 {
        let models = 4;
        let problem = problem(3, models, &mut rng);
        let belief = Belief::new((0..models).map(|_| rng.between(0.1, 1.0)).collect())
            .expect("positive belief");
        let acquisition = Acquisition::binary(
            "assay",
            0.0,
            (0..models).map(|_| rng.between(0.02, 0.98)).collect(),
        )
        .expect("proper acquisition");

        let value = value_of_information(&problem, &belief, &acquisition).expect("priceable");
        assert!(
            value.gross >= -1e-12,
            "information cannot hurt a decider free to ignore it, got {}",
            value.gross
        );
    }
}

#[test]
fn an_uninformative_acquisition_is_worthless_to_within_the_last_bit_of_the_bayes_risk() {
    let mut rng = SplitMix64::new(7);
    let problem = problem(3, 4, &mut rng);
    let belief = Belief::new(vec![0.4, 0.3, 0.2, 0.1]).expect("belief");
    let acquisition = Acquisition::uninformative("blind", 0.0, 4).expect("proper");

    let value = value_of_information(&problem, &belief, &acquisition).expect("priceable");
    assert!(
        value.gross.abs() < 1e-15,
        "an outcome distribution identical under every model is worthless; the residual is the \
         last bit of the Bayes-risk expectation, not a real value, and it was {}",
        value.gross
    );
    assert!(
        !value.changes_the_action(),
        "and the action-invariance check is exact, so it is the one to decide on"
    );
}

#[test]
fn the_net_value_is_negative_when_the_burden_exceeds_what_the_answer_buys() {
    let problem = DecisionProblem::new(
        vec!["treat".into(), "withhold".into()],
        vec!["present".into(), "absent".into()],
        vec![0.0, 1.0, 1.0, 0.0],
    )
    .expect("well-formed");
    let belief = Belief::uniform(2).expect("uniform");
    let acquisition = Acquisition::binary("costly_biopsy", 10.0, vec![0.9, 0.1]).expect("proper");

    let value = value_of_information(&problem, &belief, &acquisition).expect("priceable");
    assert!(value.gross > 0.0, "the test is informative");
    assert!(
        value.net < 0.0 && !value.worth_acquiring(),
        "43.50 requires specimen and human burden to enter the valuation"
    );
}

#[test]
fn an_acquisition_that_cannot_change_the_action_is_worth_nothing_however_informative() {
    let problem = DecisionProblem::new(
        vec!["always_best".into(), "never_best".into()],
        vec!["m0".into(), "m1".into()],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .expect("well-formed");
    let belief = Belief::uniform(2).expect("uniform");
    let acquisition = Acquisition::binary("perfect_test", 0.0, vec![1.0, 0.0]).expect("proper");

    let value = value_of_information(&problem, &belief, &acquisition).expect("priceable");
    assert_eq!(
        value.gross, 0.0,
        "a test that resolves the model but not the decision buys nothing"
    );
    assert!(!value.changes_the_action());
}

#[test]
fn the_value_of_two_acquisitions_together_can_exceed_the_sum_of_their_individual_values() {
    let problem = DecisionProblem::new(
        vec!["call".into(), "do_not_call".into()],
        vec!["hi_gain".into(), "hi_flat".into(), "lo_gain".into(), "lo_flat".into()],
        vec![0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
    )
    .expect("well-formed");
    let belief = Belief::new(vec![0.2, 0.3, 0.3, 0.2]).expect("belief");

    let purity = Acquisition::binary("purity", 0.0, vec![0.98, 0.98, 0.02, 0.02]).expect("proper");
    let copy_number =
        Acquisition::binary("copy_number", 0.0, vec![0.98, 0.02, 0.98, 0.02]).expect("proper");

    let report = complementarity(&problem, &belief, &[purity, copy_number]).expect("priceable");
    assert!(
        report.is_complementary(),
        "43.14's micro-example: each bit alone is worthless, the pair is decisive. joint {} vs sum {}",
        report.joint_gross,
        report.sum_of_singletons
    );
    assert_eq!(
        report.sum_of_singletons, 0.0,
        "neither single bit moves the action, so both price at zero"
    );
    assert!(report.joint_gross > 0.0);
}

#[test]
fn an_acquisition_whose_outcome_likelihoods_do_not_sum_to_one_is_rejected() {
    let outcome = Acquisition::new(
        "leaky",
        0.0,
        vec![
            Outcome::new("positive", vec![0.6, 0.3]),
            Outcome::new("negative", vec![0.2, 0.3]),
        ],
        2,
    );
    assert!(matches!(
        outcome,
        Err(EpistemicError::ImproperAcquisition { .. })
    ));
}

#[test]
fn an_acquisition_with_no_outcomes_is_rejected() {
    assert!(matches!(
        Acquisition::new("void", 0.0, vec![], 2),
        Err(EpistemicError::OutcomelessAcquisition { .. })
    ));
}

#[test]
fn the_joint_value_of_an_empty_bundle_is_exactly_zero() {
    let mut rng = SplitMix64::new(11);
    let problem = problem(3, 3, &mut rng);
    let belief = Belief::uniform(3).expect("uniform");
    let value = joint_value(&problem, &belief, &[]).expect("priceable");
    assert_eq!(value.gross, 0.0);
    assert_eq!(value.cost, 0.0);
}

#[test]
fn two_replicates_of_the_same_test_are_worth_far_less_than_twice_one() {
    let problem = DecisionProblem::new(
        vec!["a".into(), "b".into()],
        vec!["m0".into(), "m1".into()],
        vec![0.0, 1.0, 1.0, 0.0],
    )
    .expect("well-formed");
    let belief = Belief::uniform(2).expect("uniform");
    let one = Acquisition::binary("one", 0.0, vec![0.8, 0.2]).expect("proper");
    let two = Acquisition::binary("two", 0.0, vec![0.8, 0.2]).expect("proper");

    let joint = joint_value(&problem, &belief, &[one.clone(), two.clone()]).expect("priceable");
    let single = value_of_information(&problem, &belief, &one).expect("priceable");
    let report = complementarity(&problem, &belief, &[one, two]).expect("priceable");

    assert!(single.gross > 0.0, "the single test is informative");
    assert!(
        joint.gross >= single.gross - 1e-12,
        "a bundle is never worth less than one of its members"
    );
    assert!(
        (joint.gross - single.gross).abs() < 1e-12,
        "and here the second replicate is worth exactly nothing more: the first already carries \
         the posterior across the action boundary, so the pair and the singleton price the same. \
         joint {} against single {}",
        joint.gross,
        single.gross
    );
    assert!(
        !report.is_complementary() && report.excess < 0.0,
        "summing the members would have doubled a value the bundle does not have; this is the \
         diminishing-returns case, the mirror of the complementary one"
    );
}

#[test]
fn an_acquisition_priced_against_a_belief_of_the_wrong_width_is_rejected() {
    let problem = DecisionProblem::new(
        vec!["a".into()],
        vec!["m0".into(), "m1".into(), "m2".into()],
        vec![0.0, 1.0, 1.0],
    )
    .expect("well-formed");
    let belief = Belief::uniform(2).expect("uniform");
    let acquisition = Acquisition::binary("t", 0.0, vec![0.5, 0.5]).expect("proper");
    assert!(matches!(
        value_of_information(&problem, &belief, &acquisition),
        Err(EpistemicError::BeliefShape { .. })
    ));
}
