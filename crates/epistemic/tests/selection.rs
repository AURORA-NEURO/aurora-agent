//! Blueprint 43.14: submodularity checked by exhaustion, and the ratio measured against brute force.

use bioprism_epistemic::greedy::{greedy, lazy_greedy, Constraint};
use bioprism_epistemic::objective::{
    Coverage, ExpectedRiskReduction, HypothesisElimination, RegretReduction, SetFunction, Tabulated,
};
use bioprism_epistemic::optimal::{
    brute_force_optimum, complementary_instance, coverage_family, measure_ratio,
    misleading_instance, regret_family,
};
use bioprism_epistemic::submodularity::{check, MAX_GLOBAL_GROUND};
use bioprism_epistemic::theorem::{Applicability, Guarantee, ONE_MINUS_ONE_OVER_E};
use bioprism_epistemic::{Acquisition, Belief, DecisionProblem, EpistemicError};
use std::collections::BTreeSet;

#[test]
fn weighted_coverage_passes_the_exhaustive_monotone_submodular_check() {
    for (function, _) in coverage_family(0xC0FFEE, 12, 8, 10).expect("generable") {
        let report = check(&function).expect("checkable");
        assert!(
            report.monotone_submodular(),
            "coverage is monotone submodular by construction; the checker disagreed: {:?}",
            report.why_no_guarantee()
        );
        assert!(report.global_triples > 0 && report.local_triples > 0);
    }
}

#[test]
fn the_local_and_global_submodularity_characterisations_agree_on_every_generated_instance() {
    for (function, _) in coverage_family(0xA11CE, 10, 8, 8).expect("generable") {
        let report = check(&function).expect("checkable");
        assert!(report.characterisations_agree());
    }
    for instance in regret_family(0xD15EA5E, 25, 7, 4, 3).expect("generable") {
        let (function, _) = instance.tabulate().expect("tabulable");
        let report = check(&function).expect("checkable");
        assert!(
            report.characterisations_agree(),
            "the cheap local form and the full A-subset-of-B form must reach the same verdict; \
             a disagreement is a bug in the checker, not a property of the function"
        );
    }
}

#[test]
fn the_greedy_guarantee_is_checked_against_brute_force_optimal_not_asserted() {
    let instances = coverage_family(0x5EED_1234, 40, 9, 12).expect("generable");
    let measurement = measure_ratio("weighted-coverage", &instances).expect("measurable");

    assert_eq!(measurement.instances, 40);
    assert_eq!(
        measurement.passed_submodularity, 40,
        "every coverage instance must clear the exhaustive check"
    );
    assert!(
        measurement.respects_theoretical_factor(),
        "greedy fell below 1-1/e on a monotone submodular instance, which would falsify the \
         guarantee: worst was {:?}",
        measurement.worst
    );
    assert!(
        measurement.min_ratio >= ONE_MINUS_ONE_OVER_E,
        "measured worst-case ratio {} is below the theoretical floor {ONE_MINUS_ONE_OVER_E}",
        measurement.min_ratio
    );
    assert!(
        measurement.min_ratio > 0.9,
        "and it clears the floor by a wide margin: the bound is tight only on adversarial \
         constructions, so quoting 1-1/e as expected performance would understate greedy just as \
         quoting the measured {} as a guarantee would overstate it",
        measurement.min_ratio
    );
}

#[test]
fn regret_reduction_fails_the_submodularity_check_with_a_named_witness() {
    let instance = complementary_instance().expect("constructible");
    let (function, _) = instance.tabulate().expect("tabulable");
    let report = check(&function).expect("checkable");

    let violation = report
        .global_violation
        .as_ref()
        .expect("the decisive-pair construction must violate diminishing returns");
    assert!(
        violation.excess > 0.5,
        "the violation should be large: gain {} on {:?} against {} on {:?}",
        violation.gain_on_smaller,
        violation.smaller,
        violation.gain_on_larger,
        violation.larger
    );
    assert!(!report.monotone_submodular());
    assert!(report.why_no_guarantee().is_some());
}

#[test]
fn regret_reduction_fails_the_monotonicity_check_on_the_misleading_instance() {
    let instance = misleading_instance().expect("constructible");
    let (function, _) = instance.tabulate().expect("tabulable");
    let report = check(&function).expect("checkable");

    let violation = report
        .monotone_violation
        .as_ref()
        .expect("an item that moves the action away from the full-evidence action lowers the value");
    assert!(violation.drop > 0.0);
    assert!(
        report.normalised,
        "the objective is still normalised; only monotonicity failed"
    );
}

#[test]
fn no_approximation_factor_is_quoted_without_a_submodularity_report() {
    let (function, constraint) = coverage_family(1, 1, 6, 6)
        .expect("generable")
        .pop()
        .expect("one instance");
    let selection = greedy(&function, &constraint, &BTreeSet::new(), None).expect("selectable");

    assert!(
        matches!(selection.guarantee, Applicability::NotChecked { .. }),
        "a factor with no check behind it is a number, not a guarantee: {:?}",
        selection.guarantee
    );
    assert_eq!(selection.guarantee.factor(), None);
}

#[test]
fn no_approximation_factor_is_quoted_when_the_check_failed() {
    let instance = complementary_instance().expect("constructible");
    let (function, constraint) = instance.tabulate().expect("tabulable");
    let report = check(&function).expect("checkable");
    let selection =
        greedy(&function, &constraint, &BTreeSet::new(), Some(&report)).expect("selectable");

    match &selection.guarantee {
        Applicability::DoesNotApply {
            guarantee,
            failed_precondition,
        } => {
            assert_eq!(*guarantee, Guarantee::GreedyCardinality);
            assert!(!failed_precondition.is_empty());
        }
        other => panic!("expected a reasoned refusal, got {other:?}"),
    }
    assert_eq!(selection.guarantee.factor(), None);
}

#[test]
fn greedy_selects_nothing_on_the_complementary_instance_and_brute_force_finds_the_pair() {
    let instance = complementary_instance().expect("constructible");
    let (function, constraint) = instance.tabulate().expect("tabulable");
    let protected = BTreeSet::new();

    let selection = greedy(&function, &constraint, &protected, None).expect("selectable");
    let (optimal_set, optimal_value) =
        brute_force_optimum(&function, &constraint, &protected).expect("enumerable");

    assert!(
        selection.chosen.is_empty(),
        "every first-step marginal is zero, so greedy takes no step at all; it chose {:?}",
        selection.chosen
    );
    assert_eq!(selection.value, 0.0);
    assert!(
        optimal_value > 0.8,
        "the decisive pair recovers nearly the whole decision loss, got {optimal_value}"
    );
    assert_eq!(
        optimal_set.len(),
        2,
        "the optimum is the purity/copy-number pair"
    );
}

#[test]
fn the_measured_worst_case_ratio_on_the_regret_family_falls_below_one_minus_one_over_e() {
    let mut instances = Vec::new();
    for instance in regret_family(0x9E37_79B9, 60, 8, 4, 3).expect("generable") {
        instances.push(instance.tabulate().expect("tabulable"));
    }
    let complementary = complementary_instance().expect("constructible");
    instances.push(complementary.tabulate().expect("tabulable"));

    let measurement = measure_ratio("regret-reduction", &instances).expect("measurable");
    assert_eq!(measurement.instances, 61);
    assert!(
        measurement.below_theoretical > 0,
        "the objective a context compiler actually wants must be shown to land below the \
         guarantee's floor, not merely to lack a proof of it"
    );
    assert!(
        measurement.min_ratio < ONE_MINUS_ONE_OVER_E,
        "measured minimum was {}, which does not demonstrate the gap",
        measurement.min_ratio
    );
    assert_eq!(
        measurement.min_ratio, 0.0,
        "the complementary instance is in this family and greedy takes nothing on it"
    );
    assert_eq!(
        measurement.degenerate, 43,
        "43 of the 61 instances have an optimum of zero — no subset within the bound moves the \
         Bayes action off the prior's choice — so they score 1.0 by convention and carry no \
         information about greedy. The mean over this family is therefore not a statistic; the \
         minimum is."
    );
}

#[test]
fn regret_reduction_is_submodular_on_some_instances_and_not_on_others() {
    let instances: Vec<_> = regret_family(0x9E37_79B9, 60, 8, 4, 3)
        .expect("generable")
        .iter()
        .map(|i| i.tabulate().expect("tabulable"))
        .collect();
    let measurement = measure_ratio("regret-reduction", &instances).expect("measurable");

    assert!(
        measurement.passed_submodularity > 0,
        "an objective that failed the check on every random draw would make the check trivially \
         conservative; it does not"
    );
    assert!(
        measurement.passed_submodularity < measurement.instances,
        "and one that passed on every draw would make the counterexamples flukes; it does not"
    );
    assert_eq!(
        (measurement.instances, measurement.passed_submodularity),
        (60, 17),
        "the measured split on seed 0x9E3779B9: 17 of 60 random decision instances happen to be \
         monotone submodular and 43 are not. This is the honest shape of the finding — regret \
         reduction is not submodular *in general*, which is exactly why the condition has to be \
         checked per instance rather than assumed per objective. If this number moves, the \
         generator or the checker changed and the claim needs re-reading, not re-baselining."
    );
}

#[test]
fn lazy_greedy_returns_the_identical_set_when_the_objective_passed_the_check() {
    for (function, constraint) in coverage_family(0xFEED_BEEF, 30, 9, 11).expect("generable") {
        let report = check(&function).expect("checkable");
        assert!(report.monotone_submodular());
        let plain = greedy(&function, &constraint, &BTreeSet::new(), Some(&report))
            .expect("selectable");
        let lazy = lazy_greedy(&function, &constraint, &BTreeSet::new(), Some(&report))
            .expect("selectable");
        assert_eq!(
            plain.chosen, lazy.chosen,
            "under submodularity a stale marginal is an upper bound, so the lazy front-runner is \
             the true front-runner and the two selections coincide"
        );
        assert_eq!(plain.value, lazy.value);
    }
}

#[test]
fn a_protected_closure_larger_than_the_cardinality_bound_is_refused_not_trimmed() {
    let (function, _) = coverage_family(3, 1, 6, 6)
        .expect("generable")
        .pop()
        .expect("one instance");
    let constraint = Constraint::cardinality(2, 6).expect("valid");
    let protected = BTreeSet::from([0, 1, 2, 3]);

    assert!(matches!(
        greedy(&function, &constraint, &protected, None),
        Err(EpistemicError::ProtectedClosureExceedsCardinality { .. })
    ));
}

#[test]
fn a_protected_closure_over_budget_is_refused_not_trimmed() {
    let (function, _) = coverage_family(4, 1, 6, 6)
        .expect("generable")
        .pop()
        .expect("one instance");
    let constraint = Constraint::knapsack(2.0, vec![1.0; 6]).expect("valid");
    let protected = BTreeSet::from([0, 1, 2]);

    assert!(matches!(
        greedy(&function, &constraint, &protected, None),
        Err(EpistemicError::ProtectedClosureExceedsBudget { .. })
    ));
}

#[test]
fn the_protected_closure_is_present_in_every_selection_whatever_relevance_found() {
    let covers = vec![
        BTreeSet::from([0, 1, 2, 3]),
        BTreeSet::from([4]),
        BTreeSet::new(),
        BTreeSet::from([5]),
    ];
    let function = Coverage::uniform("closure-test", covers, 6).expect("valid");
    let table = Tabulated::of(&function).expect("tabulable");
    let constraint = Constraint::cardinality(2, 4).expect("valid");
    let protected = BTreeSet::from([2]);

    let selection = greedy(&table, &constraint, &protected, None).expect("selectable");
    assert!(
        selection.chosen.contains(&2),
        "element 2 covers nothing and must still be in the selection; closure precedes relevance"
    );
    assert_eq!(selection.protected, vec![2]);
}

#[test]
fn the_knapsack_case_reports_no_factor_and_names_why() {
    let (function, _) = coverage_family(5, 1, 6, 6)
        .expect("generable")
        .pop()
        .expect("one instance");
    let constraint = Constraint::knapsack(3.0, vec![1.0, 2.0, 1.5, 0.5, 1.0, 2.5]).expect("valid");
    let report = check(&function).expect("checkable");
    let selection =
        greedy(&function, &constraint, &BTreeSet::new(), Some(&report)).expect("selectable");

    match &selection.guarantee {
        Applicability::DoesNotApply { guarantee, .. } => {
            assert_eq!(*guarantee, Guarantee::GreedyKnapsackCostBenefit);
        }
        other => panic!("a passing submodularity check must not buy a knapsack factor: {other:?}"),
    }
    assert!(selection.cost <= 3.0 + 1e-9);
}

#[test]
fn hypothesis_elimination_is_a_coverage_function_and_passes_the_check() {
    let pool = bioprism_epistemic::EvidencePool::new(vec![
        bioprism_epistemic::EvidenceItem::new("a", 1.0, vec![1.0, 0.0, 0.0, 1.0]).expect("item"),
        bioprism_epistemic::EvidenceItem::new("b", 1.0, vec![0.0, 1.0, 1.0, 1.0]).expect("item"),
        bioprism_epistemic::EvidenceItem::new("c", 1.0, vec![1.0, 1.0, 0.0, 0.0]).expect("item"),
    ])
    .expect("pool");
    let function = HypothesisElimination::from_pool(&pool, 4).expect("buildable");
    let report = check(&function).expect("checkable");
    assert!(
        report.monotone_submodular(),
        "eliminating hypotheses is coverage over the eliminated set: {:?}",
        report.why_no_guarantee()
    );
}

#[test]
fn expected_risk_reduction_is_monotone_and_not_submodular() {
    let problem = DecisionProblem::new(
        vec!["call".into(), "do_not_call".into()],
        vec!["hi_gain".into(), "hi_flat".into(), "lo_gain".into(), "lo_flat".into()],
        vec![0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
    )
    .expect("well-formed");
    let belief = Belief::new(vec![0.2, 0.3, 0.3, 0.2]).expect("belief");
    let acquisitions = vec![
        Acquisition::binary("purity", 0.0, vec![0.98, 0.98, 0.02, 0.02]).expect("proper"),
        Acquisition::binary("copy_number", 0.0, vec![0.98, 0.02, 0.98, 0.02]).expect("proper"),
        Acquisition::binary("expression", 0.0, vec![0.7, 0.5, 0.5, 0.3]).expect("proper"),
    ];
    let function =
        ExpectedRiskReduction::new(&problem, &belief, &acquisitions).expect("constructible");
    let report = check(&function).expect("checkable");

    assert!(
        report.monotone_violation.is_none(),
        "expected risk reduction is monotone: information never hurts in expectation"
    );
    assert!(
        report.global_violation.is_some(),
        "and it is not submodular, which is the pairing that matters: passing the easy check is \
         not passing the guarantee's precondition"
    );
    assert!(!report.monotone_submodular());
}

#[test]
fn the_submodularity_check_refuses_rather_than_sampling_above_its_cap() {
    let covers: Vec<BTreeSet<usize>> = (0..MAX_GLOBAL_GROUND + 1)
        .map(|i| BTreeSet::from([i % 4]))
        .collect();
    let function = Coverage::uniform("too-big", covers, 4).expect("valid");
    assert!(matches!(
        check(&function),
        Err(EpistemicError::ExhaustiveCapExceeded { .. })
    ));
}

#[test]
fn a_knapsack_constraint_with_a_free_item_is_refused_rather_than_dividing_by_zero() {
    assert!(
        matches!(
            Constraint::knapsack(3.0, vec![1.0, 0.0, 2.0]),
            Err(EpistemicError::InadmissibleCost { .. })
        ),
        "cost-benefit greedy divides by cost; a zero-cost item would rank at infinity"
    );
    assert!(matches!(
        Constraint::knapsack(f64::NAN, vec![1.0; 3]),
        Err(EpistemicError::InadmissibleCost { .. })
    ));
}

#[test]
fn regret_reduction_is_normalised_so_the_empty_context_scores_exactly_zero() {
    for instance in regret_family(0x1111_2222, 20, 6, 4, 3).expect("generable") {
        let function =
            RegretReduction::new(&instance.problem, &instance.prior, &instance.pool)
                .expect("constructible");
        assert_eq!(
            function.value(&BTreeSet::new()).expect("evaluable"),
            0.0,
            "F(empty) must be exactly zero or the 1-1/e factor is meaningless even where it applies"
        );
    }
}
