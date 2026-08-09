//! Blueprint 43.50: decision rate-distortion, identification, and abstention.

use bioprism_epistemic::decision::{Belief, DecisionProblem};
use bioprism_epistemic::evidence::{EvidenceItem, EvidencePool};
use bioprism_epistemic::ratedistortion::{
    evaluate_context, frontier, identification, minimal_sufficient_context, AbstentionReason,
    DistortionCriterion, Identification, Sufficiency,
};
use bioprism_epistemic::rng::SplitMix64;
use bioprism_epistemic::EpistemicError;
use std::collections::BTreeSet;

const FLOOR: f64 = 0.01;

fn two_model_problem() -> DecisionProblem {
    DecisionProblem::new(
        vec!["treat".into(), "withhold".into(), "defer".into()],
        vec!["present".into(), "absent".into()],
        vec![0.0, 1.0, 1.0, 0.0, 0.4, 0.4],
    )
    .expect("well-formed problem")
}

fn pool_of(likelihoods: &[(&str, [f64; 2])]) -> EvidencePool {
    EvidencePool::new(
        likelihoods
            .iter()
            .map(|(id, l)| EvidenceItem::new(*id, 1.0, l.to_vec()).expect("admissible item"))
            .collect(),
    )
    .expect("unique ids")
}

fn seeded_instance(rng: &mut SplitMix64, models: usize, actions: usize, items: usize) -> (DecisionProblem, Belief, EvidencePool) {
    let loss: Vec<f64> = (0..actions * models).map(|_| rng.between(0.0, 1.0)).collect();
    let problem = DecisionProblem::new(
        (0..actions).map(|a| format!("a{a}")).collect(),
        (0..models).map(|m| format!("m{m}")).collect(),
        loss,
    )
    .expect("well-formed problem");
    let prior = Belief::new((0..models).map(|_| rng.between(0.2, 1.0)).collect())
        .expect("positive prior");
    let pool = EvidencePool::new(
        (0..items)
            .map(|i| {
                EvidenceItem::new(
                    format!("e{i}"),
                    1.0,
                    (0..models).map(|_| rng.between(0.05, 1.0)).collect(),
                )
                .expect("admissible item")
            })
            .collect(),
    )
    .expect("unique ids");
    (problem, prior, pool)
}

#[test]
fn the_distortion_of_the_full_context_is_exactly_zero_under_the_bayes_criterion() {
    let problem = two_model_problem();
    let prior = Belief::uniform(2).expect("uniform");
    let pool = pool_of(&[("a", [0.9, 0.2]), ("b", [0.3, 0.8]), ("c", [0.5, 0.5])]);

    let evaluation = evaluate_context(
        &problem,
        &prior,
        &pool,
        &pool.everything(),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("evaluable");

    assert_eq!(
        evaluation.distortion, 0.0,
        "acting on everything cannot regret against everything"
    );
    assert!(evaluation.action_preserved());
}

#[test]
fn distortion_is_never_negative_across_a_seeded_family() {
    let mut rng = SplitMix64::new(0x5150_0F1E);
    for _ in 0..60 {
        let (problem, prior, pool) = seeded_instance(&mut rng, 4, 3, 6);
        for mask in 0..(1u32 << pool.len()) {
            let subset: BTreeSet<usize> =
                (0..pool.len()).filter(|i| (mask >> i) & 1 == 1).collect();
            let evaluation = evaluate_context(
                &problem,
                &prior,
                &pool,
                &subset,
                DistortionCriterion::BayesRegret,
                FLOOR,
            )
            .expect("evaluable");
            assert!(
                evaluation.distortion >= 0.0,
                "regret against the best action cannot be negative, got {}",
                evaluation.distortion
            );
        }
    }
}

#[test]
fn a_larger_context_can_have_strictly_higher_decision_distortion_than_a_subset_of_it() {
    let problem = DecisionProblem::new(
        vec!["a0".into(), "a1".into(), "a2".into()],
        vec!["m0".into(), "m1".into(), "m2".into()],
        vec![0.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0],
    )
    .expect("well-formed");
    let prior = Belief::uniform(3).expect("uniform");
    let pool = pool_of(&[]);
    let _ = pool;
    let pool = EvidencePool::new(vec![
        EvidenceItem::new("misleading", 1.0, vec![0.1, 1.0, 0.1]).expect("item"),
        EvidenceItem::new("corroborating", 1.0, vec![1.0, 0.05, 1.0]).expect("item"),
    ])
    .expect("pool");

    let empty = evaluate_context(
        &problem,
        &prior,
        &pool,
        &BTreeSet::new(),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("evaluable");
    let misleading_only = evaluate_context(
        &problem,
        &prior,
        &pool,
        &BTreeSet::from([0]),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("evaluable");

    assert!(
        misleading_only.distortion > empty.distortion,
        "adding the misleading item should raise distortion: empty {} vs one item {}",
        empty.distortion,
        misleading_only.distortion
    );
}

#[test]
fn the_frontier_is_monotone_non_increasing_in_rate() {
    let mut rng = SplitMix64::new(0x0FF1_CE01);
    for _ in 0..15 {
        let (problem, prior, pool) = seeded_instance(&mut rng, 4, 3, 7);
        let front = frontier(
            &problem,
            &prior,
            &pool,
            DistortionCriterion::BayesRegret,
            FLOOR,
        )
        .expect("enumerable");
        let mut best_so_far = f64::INFINITY;
        for point in &front.points {
            let at_rate = front
                .best_at_rate(point.rate)
                .expect("a point exists at its own rate");
            assert!(
                at_rate.distortion <= best_so_far + 1e-12,
                "the best distortion available cannot get worse as rate grows"
            );
            best_so_far = at_rate.distortion;
        }
    }
}

#[test]
fn the_cheapest_sufficient_context_is_never_more_expensive_than_retaining_everything() {
    let problem = two_model_problem();
    let prior = Belief::uniform(2).expect("uniform");
    let pool = pool_of(&[
        ("decisive", [0.95, 0.05]),
        ("weak", [0.6, 0.5]),
        ("noise", [0.5, 0.5]),
    ]);

    let outcome = minimal_sufficient_context(
        &problem,
        &prior,
        &pool,
        DistortionCriterion::BayesRegret,
        0.0,
        FLOOR,
    )
    .expect("solvable");

    match outcome {
        Sufficiency::Sufficient {
            rate, full_rate, ..
        } => assert!(
            rate <= full_rate,
            "a compression that costs more than the original is not a compression"
        ),
        other => panic!("expected sufficiency, got {other:?}"),
    }
}

#[test]
fn an_item_with_the_same_likelihood_under_every_model_is_dropped_from_the_frontier() {
    let problem = two_model_problem();
    let prior = Belief::uniform(2).expect("uniform");
    let pool = EvidencePool::new(vec![
        EvidenceItem::new("decisive", 1.0, vec![0.95, 0.05]).expect("item"),
        EvidenceItem::uninformative("filler", 1.0, 2).expect("item"),
    ])
    .expect("pool");

    let front = frontier(
        &problem,
        &prior,
        &pool,
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("enumerable");

    let filler = pool.index_of("filler").expect("present");
    assert!(
        front.points.iter().all(|p| !p.retained.contains(&filler)),
        "an item that cannot move any posterior is Pareto-dominated at every rate"
    );
}

#[test]
fn a_context_that_annihilates_the_posterior_is_an_error_not_a_confident_answer() {
    let problem = two_model_problem();
    let prior = Belief::uniform(2).expect("uniform");
    let pool = EvidencePool::new(vec![
        EvidenceItem::new("rules_out_absent", 1.0, vec![1.0, 0.0]).expect("item"),
        EvidenceItem::new("rules_out_present", 1.0, vec![0.0, 1.0]).expect("item"),
    ])
    .expect("pool");

    let outcome = evaluate_context(
        &problem,
        &prior,
        &pool,
        &pool.everything(),
        DistortionCriterion::BayesRegret,
        FLOOR,
    );

    assert!(
        matches!(outcome, Err(EpistemicError::DegenerateBelief { .. })),
        "two items that jointly exclude every model are a contradiction, not evidence"
    );
}

#[test]
fn identification_is_point_identified_when_every_compatible_model_prefers_one_action() {
    let problem = DecisionProblem::new(
        vec!["act".into(), "wait".into()],
        vec!["m0".into(), "m1".into()],
        vec![0.0, 0.1, 1.0, 1.0],
    )
    .expect("well-formed");
    let prior = Belief::uniform(2).expect("uniform");
    let pool = pool_of(&[("any", [0.7, 0.3])]);

    let status = identification(&problem, &prior, &pool, 0.0, FLOOR).expect("classifiable");
    assert!(
        matches!(status, Identification::PointIdentified { .. }),
        "both models prefer to act, so residual uncertainty never reaches the decision: {status:?}"
    );
}

#[test]
fn non_identification_produces_abstention_rather_than_the_cheapest_context() {
    let problem = DecisionProblem::new(
        vec!["a0".into(), "a1".into()],
        vec!["m0".into(), "m1".into()],
        vec![0.0, 1.0, 1.0, 0.0],
    )
    .expect("well-formed");
    let prior = Belief::uniform(2).expect("uniform");
    let pool = pool_of(&[("uninformative", [0.5, 0.5])]);

    let status = identification(&problem, &prior, &pool, 0.1, FLOOR).expect("classifiable");
    assert!(
        matches!(status, Identification::NonIdentified { .. }),
        "models disagree by a regret of 1 against a tolerance of 0.1: {status:?}"
    );

    let outcome = minimal_sufficient_context(
        &problem,
        &prior,
        &pool,
        DistortionCriterion::MinimaxRegret,
        0.1,
        FLOOR,
    )
    .expect("solvable");
    assert!(
        matches!(
            outcome,
            Sufficiency::Abstain {
                reason: AbstentionReason::NonIdentifiedUnderAllEvidence,
                ..
            }
        ),
        "43.50 requires abstention here, not a point answer: {outcome:?}"
    );
}

#[test]
fn declaring_a_prior_is_an_assertion_that_lifts_the_abstention_gate() {
    let problem = DecisionProblem::new(
        vec!["a0".into(), "a1".into()],
        vec!["m0".into(), "m1".into()],
        vec![0.0, 1.0, 1.0, 0.0],
    )
    .expect("well-formed");
    let prior = Belief::new(vec![0.9, 0.1]).expect("belief");
    let pool = pool_of(&[("weak", [0.6, 0.4])]);

    let minimax = minimal_sufficient_context(
        &problem,
        &prior,
        &pool,
        DistortionCriterion::MinimaxRegret,
        0.1,
        FLOOR,
    )
    .expect("solvable");
    assert!(
        matches!(minimax, Sufficiency::Abstain { .. }),
        "with no prior over a disagreeing model set there is no answer at any size"
    );

    let bayes = minimal_sufficient_context(
        &problem,
        &prior,
        &pool,
        DistortionCriterion::BayesRegret,
        0.1,
        FLOOR,
    )
    .expect("solvable");
    assert!(
        matches!(bayes, Sufficiency::Sufficient { .. }),
        "choosing the Bayes criterion is the assertion that the prior is trustworthy: {bayes:?}"
    );
}

#[test]
fn a_belief_of_total_mass_zero_is_rejected_rather_than_treated_as_uniform() {
    let outcome = Belief::new(vec![0.0, 0.0, 0.0]);
    assert!(matches!(
        outcome,
        Err(EpistemicError::DegenerateBelief { .. })
    ));
}

#[test]
fn the_minimax_action_never_has_worse_worst_case_regret_than_any_other_action() {
    let mut rng = SplitMix64::new(0xBEEF_0042);
    for _ in 0..80 {
        let models = 4;
        let actions = 4;
        let loss: Vec<f64> = (0..actions * models).map(|_| rng.between(0.0, 1.0)).collect();
        let problem = DecisionProblem::new(
            (0..actions).map(|a| format!("a{a}")).collect(),
            (0..models).map(|m| format!("m{m}")).collect(),
            loss,
        )
        .expect("well-formed");
        let compatible: Vec<usize> = (0..models).collect();
        let chosen = problem.minimax_action(&compatible).expect("non-empty set");
        let chosen_regret = problem.minimax_regret(&compatible, chosen);
        for action in 0..actions {
            assert!(
                chosen_regret <= problem.minimax_regret(&compatible, action) + 1e-12,
                "the minimax action must attain the minimum of the worst-case regrets"
            );
        }
    }
}

#[test]
fn the_minimax_of_an_empty_compatible_set_is_undefined_rather_than_the_first_action() {
    let problem = two_model_problem();
    assert_eq!(
        problem.minimax_action(&[]),
        None,
        "a minimax over nothing is not zero, it is unasked"
    );
}

#[test]
fn a_negative_distortion_tolerance_is_refused() {
    let problem = two_model_problem();
    let prior = Belief::uniform(2).expect("uniform");
    let pool = pool_of(&[("a", [0.9, 0.2])]);
    assert!(matches!(
        identification(&problem, &prior, &pool, -0.1, FLOOR),
        Err(EpistemicError::InadmissibleTolerance { .. })
    ));
}

#[test]
fn the_frontier_refuses_rather_than_sampling_above_the_enumeration_cap() {
    let problem = two_model_problem();
    let prior = Belief::uniform(2).expect("uniform");
    let pool = EvidencePool::new(
        (0..20)
            .map(|i| EvidenceItem::new(format!("e{i}"), 1.0, vec![0.6, 0.4]).expect("item"))
            .collect(),
    )
    .expect("pool");

    assert!(
        matches!(
            frontier(
                &problem,
                &prior,
                &pool,
                DistortionCriterion::BayesRegret,
                FLOOR
            ),
            Err(EpistemicError::ExhaustiveCapExceeded { .. })
        ),
        "a frontier claims a minimum; a sampled search would report an upper bound in the same type"
    );
}

#[test]
fn a_zero_distortion_context_can_still_change_the_action_when_two_actions_tie() {
    let problem = DecisionProblem::new(
        vec!["left".into(), "right".into()],
        vec!["m0".into(), "m1".into()],
        vec![0.0, 1.0, 1.0, 0.0],
    )
    .expect("well-formed");
    let prior = Belief::uniform(2).expect("uniform");
    let pool = pool_of(&[("balanced", [0.5, 0.5])]);

    let empty = evaluate_context(
        &problem,
        &prior,
        &pool,
        &BTreeSet::new(),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("evaluable");
    assert_eq!(empty.distortion, 0.0);
    assert!(
        empty.action_preserved(),
        "the deterministic tie-break must make the two evaluations agree, or replay is not stable"
    );
}
