//! The suite this crate exists for.
//!
//! Small worlds, exhaustive perturbation, and the assertion that the true influence never exceeds
//! the reported bound. A bound that has not been checked against brute force is a number, not a
//! guarantee, so a green run here is the only thing that makes the rest of the crate a claim.
//!
//! ## The one tolerance, and why it is not slack
//!
//! [`FLOAT_REORDERING`] is `1e-12`. It exists because the exact bound is computed by bucket
//! elimination and the ground truth by direct enumeration, and floating-point addition is not
//! associative — `bioprism-backends` documents that the two agree bit-for-bit only when every
//! entry and intermediate is an integer below `2^53`, which random potentials in `[0.05, 1)` are
//! not. It is a comparison allowance between two orderings of the same sum, not room for the
//! argument to be wrong: the structural bounds carry margins many orders of magnitude larger, and
//! the suite reports those margins rather than hiding inside the tolerance.

use bioprism_influence::{
    maximum_influence, smallworld, BoundMethod, Family, InfluenceAnalyzer, InfluenceError,
    Perturbation, SmallWorldSpec,
};

const FLOAT_REORDERING: f64 = 1e-12;

fn factor_ids(region: &bioprism_backends::QueryRegion) -> Vec<String> {
    region
        .factors()
        .iter()
        .map(|factor| factor.id().to_string())
        .collect()
}

#[test]
fn a_reported_bound_is_never_exceeded_by_the_true_influence() {
    let analyzer = InfluenceAnalyzer::default();
    let mut checked = 0usize;
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        for id in factor_ids(&region) {
            let analysis = analyzer
                .analyse_factor(&region, &id, &Perturbation::Removal)
                .expect("the region is well formed");
            let reported = analysis
                .estimate
                .bound()
                .unwrap_or_else(|| panic!("{} / {id}: a region with potentials must be bounded", spec.label()))
                .value();
            let truth = maximum_influence(&region, std::slice::from_ref(&id), &Perturbation::Removal, 0, 0)
                .expect("removal has exactly one realisation");
            assert!(truth.exhaustive);
            assert!(
                truth.found_influence <= reported + FLOAT_REORDERING,
                "{} / {id}: true influence {} exceeds the reported bound {reported}",
                spec.label(),
                truth.found_influence
            );
            checked += 1;
        }
    }
    assert!(checked >= 200, "only {checked} factor removals were checked");
}

#[test]
fn every_method_that_answered_is_individually_sound_not_merely_their_minimum() {
    let analyzer = InfluenceAnalyzer::default();
    let mut per_method = std::collections::BTreeMap::<&'static str, usize>::new();
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        for id in factor_ids(&region) {
            let analysis = analyzer
                .analyse_factor(&region, &id, &Perturbation::Removal)
                .expect("the region is well formed");
            let truth = maximum_influence(&region, std::slice::from_ref(&id), &Perturbation::Removal, 0, 0)
                .expect("removal has exactly one realisation")
                .found_influence;
            for outcome in &analysis.attempted {
                if let Some(value) = outcome.value {
                    assert!(
                        truth <= value + FLOAT_REORDERING,
                        "{} / {id}: method {} reported {value} but the truth is {truth}",
                        spec.label(),
                        outcome.method.as_str()
                    );
                    *per_method.entry(outcome.method.as_str()).or_default() += 1;
                }
            }
        }
    }
    assert!(per_method.contains_key("dynamic_range"));
    assert!(per_method.contains_key("chain_contraction"));
    assert!(per_method.contains_key("exact_removal"));
}

#[test]
fn a_reported_bound_is_never_exceeded_under_a_stated_multiplicative_range() {
    let analyzer = InfluenceAnalyzer::default();
    let perturbation = Perturbation::relative_tolerance(0.25).expect("a legal tolerance");
    let mut checked = 0usize;
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        for id in factor_ids(&region) {
            let analysis = analyzer
                .analyse_factor(&region, &id, &perturbation)
                .expect("the region is well formed");
            let reported = analysis
                .estimate
                .bound()
                .expect("a stated range always yields a bound")
                .value();
            match maximum_influence(&region, std::slice::from_ref(&id), &perturbation, 32, spec.seed) {
                Ok(truth) => {
                    assert!(!truth.exhaustive);
                    assert!(
                        truth.found_influence <= reported + FLOAT_REORDERING,
                        "{} / {id}: search found {} above the reported bound {reported}",
                        spec.label(),
                        truth.found_influence
                    );
                    checked += 1;
                }
                Err(InfluenceError::BruteForceTooLarge { .. }) => {}
                Err(other) => panic!("{other}"),
            }
        }
    }
    assert!(checked >= 100, "only {checked} range perturbations were searched");
}

#[test]
fn a_group_bound_is_never_exceeded_by_the_true_joint_influence() {
    let analyzer = InfluenceAnalyzer::default();
    let mut checked = 0usize;
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        let ids = factor_ids(&region);
        for size in [2usize, 3] {
            if ids.len() < size {
                continue;
            }
            let group: Vec<String> = ids.iter().take(size).cloned().collect();
            let analysis = analyzer
                .analyse_group(&region, &group, &Perturbation::Removal)
                .expect("the region is well formed");
            let reported = analysis
                .estimate
                .bound()
                .expect("a valued region always yields a group bound")
                .value();
            let truth = maximum_influence(&region, &group, &Perturbation::Removal, 0, 0)
                .expect("removal has exactly one realisation");
            assert!(
                truth.found_influence <= reported + FLOAT_REORDERING,
                "{} / {group:?}: joint influence {} exceeds the bound {reported}",
                spec.label(),
                truth.found_influence
            );
            checked += 1;
        }
    }
    assert!(checked >= 60, "only {checked} groups were checked");
}

#[test]
fn a_structural_only_group_bound_is_still_sound_when_nothing_is_executed() {
    let analyzer = InfluenceAnalyzer::default().structural_only();
    let perturbation = Perturbation::relative_tolerance(0.2).expect("a legal tolerance");
    let mut checked = 0usize;
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        let ids = factor_ids(&region);
        let group: Vec<String> = ids.iter().take(2).cloned().collect();
        if group.len() < 2 {
            continue;
        }
        let analysis = analyzer
            .analyse_group(&region, &group, &perturbation)
            .expect("the region is well formed");
        assert!(analysis
            .attempted
            .iter()
            .all(|outcome| outcome.method != BoundMethod::ExactRemoval));
        let reported = analysis.estimate.bound().expect("bounded").value();
        match maximum_influence(&region, &group, &perturbation, 24, spec.seed) {
            Ok(truth) => {
                assert!(
                    truth.found_influence <= reported + FLOAT_REORDERING,
                    "{}: structural-only bound {reported} was beaten by {}",
                    spec.label(),
                    truth.found_influence
                );
                checked += 1;
            }
            Err(InfluenceError::BruteForceTooLarge { .. }) => {}
            Err(other) => panic!("{other}"),
        }
    }
    assert!(checked >= 4);
}

#[test]
fn the_exact_removal_bound_is_tight_against_brute_force() {
    let analyzer = InfluenceAnalyzer::default();
    let spec = SmallWorldSpec {
        family: Family::Chain,
        size: 3,
        cardinality: 3,
        seed: 0x5EED_0001,
    };
    let region = smallworld::generate(&spec).expect("the family builds");
    for id in factor_ids(&region) {
        let analysis = analyzer
            .analyse_factor(&region, &id, &Perturbation::Removal)
            .expect("well formed");
        assert_eq!(
            analysis.estimate.bound().unwrap().method(),
            BoundMethod::ExactRemoval,
            "the exact method should win on a fully valued region"
        );
        let looseness = analysis.looseness().expect("an exact method ran");
        assert_eq!(looseness, 0.0);
        let truth = maximum_influence(&region, std::slice::from_ref(&id), &Perturbation::Removal, 0, 0).unwrap();
        assert!(
            (truth.found_influence - analysis.estimate.bound().unwrap().value()).abs()
                < FLOAT_REORDERING
        );
    }
}

/// The looseness measurement the task asks to be reported.
///
/// Three regimes on the same fixture, printed so a reader can see the trade rather than take it
/// on trust:
///
/// - `exact_removal` is tight by construction.
/// - `chain_contraction` stays within a small constant factor of the truth at both ends of the
///   chain: 4.8x on the factor furthest from the target, 2.4x on the one adjacent to it.
/// - `dynamic_range`, which ignores the path entirely, is 121x loose on the factor furthest from
///   the target and only 2.8x loose on the nearest one — exactly the asymmetry the propagation
///   argument exists to capture. That is the price of a bound that needs no execution and no
///   recognisable structure, and it is what a compile pass with neither would be stuck with.
#[test]
fn the_suite_reports_a_tight_bound_and_a_loose_one() {
    let spec = SmallWorldSpec {
        family: Family::Chain,
        size: 3,
        cardinality: 3,
        seed: 0x5EED_0001,
    };
    let region = smallworld::generate(&spec).expect("the family builds");
    let structural = InfluenceAnalyzer::default().structural_only();

    let far = "f.prior";
    let near = "f.t2";
    let mut report = Vec::new();
    for id in [far, near] {
        let truth = maximum_influence(&region, &[id.to_string()], &Perturbation::Removal, 0, 0)
            .unwrap()
            .found_influence;
        let dynamic =
            bioprism_influence::dynamic_range_bound(&region, id, &Perturbation::Removal).unwrap();
        let chain = structural
            .analyse_factor(&region, id, &Perturbation::Removal)
            .unwrap();
        let chain_value = chain.estimate.bound().unwrap().value();
        report.push((id, truth, dynamic.value(), chain_value));
    }

    for (id, truth, dynamic, chain) in &report {
        println!(
            "{id}: truth {truth:.9}, chain_contraction {chain:.9} (x{:.1}), dynamic_range {dynamic:.9} (x{:.1})",
            chain / truth,
            dynamic / truth
        );
        assert!(*truth <= chain + FLOAT_REORDERING);
        assert!(*chain <= dynamic + FLOAT_REORDERING);
    }

    let (_, far_truth, far_dynamic, far_chain) = report[0];
    let (_, near_truth, _, near_chain) = report[1];
    assert!(
        far_dynamic / far_truth > 50.0,
        "the path-blind bound should be badly loose far from the target, ratio was {}",
        far_dynamic / far_truth
    );
    assert!(
        far_chain / far_truth < 10.0,
        "the contraction bound should stay within a small factor far from the target, ratio was {}",
        far_chain / far_truth
    );
    assert!(
        far_dynamic / far_chain > 10.0,
        "propagation should buy an order of magnitude far from the target, it bought {}",
        far_dynamic / far_chain
    );
    assert!(
        near_chain / near_truth < 3.0,
        "the contraction bound should stay close next to the target, ratio was {}",
        near_chain / near_truth
    );
}

#[test]
fn removing_a_constant_factor_moves_the_normalised_answer_by_exactly_zero() {
    use bioprism_backends::{QueryRegion, RegionFactor};
    let region = QueryRegion::builder("constant")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::with_table("f.ab", vec!["a", "b"], vec![1.0, 2.0, 3.0, 4.0]))
        .factor(RegionFactor::with_table("f.const", vec!["a"], vec![0.5, 0.5]))
        .free("b")
        .build()
        .unwrap();
    let truth = maximum_influence(
        &region,
        &["f.const".to_string()],
        &Perturbation::Removal,
        0,
        0,
    )
    .unwrap();
    assert_eq!(truth.found_influence, 0.0);

    let analysis = InfluenceAnalyzer::default()
        .analyse_factor(&region, "f.const", &Perturbation::Removal)
        .unwrap();
    assert_eq!(analysis.estimate.bound().unwrap().value(), 0.0);
}

#[test]
fn the_two_executors_agree_on_the_unperturbed_answer() {
    use bioprism_backends::{DirectMaterialization, QueryBackend, VariableElimination};
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        let eliminated = VariableElimination::default().execute(&region).unwrap();
        let enumerated = DirectMaterialization::new().execute(&region).unwrap();
        let difference = eliminated
            .max_absolute_difference(&enumerated)
            .expect("the two answers share a scope");
        assert!(
            difference < 1e-12,
            "{}: the two executors disagree by {difference}",
            spec.label()
        );
    }
}

#[test]
fn brute_force_refuses_rather_than_sampling_when_the_space_is_too_large() {
    let spec = SmallWorldSpec {
        family: Family::Triangle,
        size: 0,
        cardinality: 3,
        seed: 1,
    };
    let region = smallworld::generate(&spec).unwrap();
    let ids = factor_ids(&region);
    let perturbation = Perturbation::relative_tolerance(0.1).unwrap();
    let error = maximum_influence(&region, &ids, &perturbation, 0, 0).unwrap_err();
    assert!(matches!(
        error,
        InfluenceError::BruteForceTooLarge { .. }
    ));
}

#[test]
fn a_vacuous_bound_is_still_never_exceeded() {
    use bioprism_backends::{QueryRegion, RegionFactor};
    let region = QueryRegion::builder("zero-entry")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::with_table("f.ab", vec!["a", "b"], vec![1.0, 2.0, 3.0, 4.0]))
        .factor(RegionFactor::with_table("f.gate", vec!["a"], vec![0.0, 1.0]))
        .free("b")
        .build()
        .unwrap();
    let structural = InfluenceAnalyzer::default().structural_only();
    let analysis = structural
        .analyse_factor(&region, "f.gate", &Perturbation::Removal)
        .unwrap();
    let reported = analysis.estimate.bound().unwrap();
    assert!(reported.is_vacuous());
    let truth = maximum_influence(&region, &["f.gate".to_string()], &Perturbation::Removal, 0, 0)
        .unwrap()
        .found_influence;
    assert!(truth <= reported.value());
    assert!(truth > 0.0, "the fixture should have real influence to bound");
}

#[test]
fn the_family_is_reproducible_from_its_seeds() {
    for spec in smallworld::family() {
        let first = smallworld::generate(&spec).unwrap();
        let second = smallworld::generate(&spec).unwrap();
        assert_eq!(first, second, "{} is not reproducible", spec.label());
    }
}

#[test]
fn different_seeds_produce_different_worlds() {
    let left = smallworld::generate(&SmallWorldSpec {
        family: Family::Chain,
        size: 3,
        cardinality: 2,
        seed: 1,
    })
    .unwrap();
    let right = smallworld::generate(&SmallWorldSpec {
        family: Family::Chain,
        size: 3,
        cardinality: 2,
        seed: 2,
    })
    .unwrap();
    assert_ne!(left.factors(), right.factors());
}
