//! Recognising the class the contraction argument is stated for, and refusing everything else.
//!
//! The refusals matter more than the acceptance. A method that quietly approximated a tree by its
//! longest path would produce a number that is not a bound, and a certificate carrying it would
//! license omitting evidence that mattered.

use bioprism_influence::{
    chain_of, dobrushin_coefficients, dynamic_range_bound, smallworld, BoundMethod, Family,
    InfluenceAnalyzer, Perturbation, SmallWorldSpec, UnknownReason,
};
use bioprism_backends::{QueryRegion, RegionFactor};

fn spec(family: Family, size: usize, cardinality: usize) -> SmallWorldSpec {
    SmallWorldSpec {
        family,
        size,
        cardinality,
        seed: 0x5EED_0001,
    }
}

fn region(family: Family, size: usize, cardinality: usize) -> QueryRegion {
    smallworld::generate(&spec(family, size, cardinality)).expect("the family builds")
}

fn refusal_detail(reason: &UnknownReason) -> String {
    match reason {
        UnknownReason::RegionOutsideMethodClass { detail, .. } => detail.clone(),
        other => panic!("expected a class refusal, got {other:?}"),
    }
}

#[test]
fn a_chain_is_recognised_with_its_variables_in_path_order() {
    let region = region(Family::Chain, 3, 2);
    let chain = chain_of(&region).expect("a generated chain is a chain");
    assert_eq!(chain.variables(), ["v0", "v1", "v2", "v3"]);
    assert_eq!(chain.transitions(), ["f.t0", "f.t1", "f.t2"]);
    assert_eq!(chain.source_factor(), "f.prior");
    assert_eq!(chain.length(), 3);
}

#[test]
fn a_chain_factors_position_is_its_distance_from_the_prior() {
    let chain = chain_of(&region(Family::Chain, 3, 2)).unwrap();
    assert_eq!(chain.position_of("f.prior"), Some(0));
    assert_eq!(chain.position_of("f.t0"), Some(1));
    assert_eq!(chain.position_of("f.t2"), Some(3));
    assert_eq!(chain.position_of("f.absent"), None);
}

#[test]
fn a_star_is_refused_because_the_centre_branches() {
    let error = chain_of(&region(Family::Star, 3, 2)).unwrap_err();
    let detail = refusal_detail(&error);
    assert!(
        detail.contains("arity-one factors") || detail.contains("degree") || detail.contains("transitions over"),
        "the refusal should name the structural clause that failed, got {detail:?}"
    );
}

#[test]
fn a_tree_is_refused_even_though_a_path_to_the_target_exists() {
    let error = chain_of(&region(Family::Tree, 2, 2)).unwrap_err();
    refusal_detail(&error);
}

#[test]
fn a_triangle_is_refused_because_dobrushin_does_not_compose_around_a_cycle() {
    let error = chain_of(&region(Family::Triangle, 0, 2)).unwrap_err();
    let detail = refusal_detail(&error);
    assert!(
        detail.contains("arity-one") || detail.contains("cycle") || detail.contains("path"),
        "got {detail:?}"
    );
}

#[test]
fn a_region_without_potentials_is_refused_by_the_chain_method() {
    let structural = QueryRegion::builder("no-tables")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::structural("f.prior", vec!["a"]))
        .factor(RegionFactor::structural("f.ab", vec!["a", "b"]))
        .free("b")
        .build()
        .unwrap();
    let detail = refusal_detail(&chain_of(&structural).unwrap_err());
    assert!(detail.contains("no potential"), "got {detail:?}");
}

#[test]
fn a_chain_whose_transitions_are_not_stochastic_is_refused() {
    let region = QueryRegion::builder("unnormalised")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::with_table("f.prior", vec!["a"], vec![0.5, 0.5]))
        .factor(RegionFactor::with_table(
            "f.ab",
            vec!["a", "b"],
            vec![1.0, 2.0, 3.0, 4.0],
        ))
        .free("b")
        .build()
        .unwrap();
    let detail = refusal_detail(&chain_of(&region).unwrap_err());
    assert!(detail.contains("row summing to"), "got {detail:?}");
}

#[test]
fn a_query_with_two_free_variables_is_refused() {
    let region = QueryRegion::builder("two-free")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::with_table("f.prior", vec!["a"], vec![0.5, 0.5]))
        .factor(RegionFactor::with_table(
            "f.ab",
            vec!["a", "b"],
            vec![0.5, 0.5, 0.5, 0.5],
        ))
        .free("a")
        .free("b")
        .build()
        .unwrap();
    let detail = refusal_detail(&chain_of(&region).unwrap_err());
    assert!(detail.contains("free variables"), "got {detail:?}");
}

#[test]
fn a_dobrushin_coefficient_lies_in_the_unit_interval() {
    for cardinality in [2usize, 3] {
        let region = region(Family::Chain, 5, cardinality);
        let chain = chain_of(&region).unwrap();
        let coefficients = dobrushin_coefficients(&region, &chain).unwrap();
        assert_eq!(coefficients.len(), chain.length());
        for delta in coefficients {
            assert!((0.0..=1.0).contains(&delta), "delta {delta} is out of range");
        }
    }
}

#[test]
fn a_kernel_with_identical_rows_has_a_dobrushin_coefficient_of_zero() {
    let region = QueryRegion::builder("forgetful")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::with_table("f.prior", vec!["a"], vec![0.9, 0.1]))
        .factor(RegionFactor::with_table(
            "f.ab",
            vec!["a", "b"],
            vec![0.3, 0.7, 0.3, 0.7],
        ))
        .free("b")
        .build()
        .unwrap();
    let chain = chain_of(&region).unwrap();
    assert_eq!(dobrushin_coefficients(&region, &chain).unwrap(), vec![0.0]);
}

#[test]
fn a_forgetful_kernel_annihilates_all_upstream_influence() {
    let region = QueryRegion::builder("forgetful")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::with_table("f.prior", vec!["a"], vec![0.9, 0.1]))
        .factor(RegionFactor::with_table(
            "f.ab",
            vec!["a", "b"],
            vec![0.3, 0.7, 0.3, 0.7],
        ))
        .free("b")
        .build()
        .unwrap();
    let analysis = InfluenceAnalyzer::default()
        .structural_only()
        .analyse_factor(&region, "f.prior", &Perturbation::Removal)
        .unwrap();
    let bound = analysis.estimate.bound().unwrap();
    assert_eq!(bound.value(), 0.0);
    assert_eq!(bound.method(), BoundMethod::ChainContraction);
}

#[test]
fn the_chain_bound_is_never_looser_than_the_path_blind_bound() {
    let structural = InfluenceAnalyzer::default().structural_only();
    for cardinality in [2usize, 3] {
        let region = region(Family::Chain, 5, cardinality);
        for factor in region.factors() {
            let blind = dynamic_range_bound(&region, factor.id(), &Perturbation::Removal)
                .unwrap()
                .value();
            let reported = structural
                .analyse_factor(&region, factor.id(), &Perturbation::Removal)
                .unwrap()
                .estimate
                .bound()
                .unwrap()
                .value();
            assert!(
                reported <= blind + 1e-15,
                "{}: the analyzer reported {reported}, above the path-blind {blind}",
                factor.id()
            );
        }
    }
}

#[test]
fn the_last_transition_is_not_attenuated_because_no_path_remains() {
    let region = region(Family::Chain, 3, 3);
    let chain = chain_of(&region).unwrap();
    let coefficients = dobrushin_coefficients(&region, &chain).unwrap();
    let attenuation: f64 = coefficients.iter().skip(chain.length()).product();
    assert_eq!(attenuation, 1.0);
}

#[test]
fn attenuation_is_monotone_in_distance_from_the_target() {
    let region = region(Family::Chain, 5, 3);
    let chain = chain_of(&region).unwrap();
    let coefficients = dobrushin_coefficients(&region, &chain).unwrap();
    let mut previous = f64::INFINITY;
    for position in (0..=chain.length()).rev() {
        let attenuation: f64 = coefficients.iter().skip(position).product();
        assert!(
            attenuation <= previous + 1e-15,
            "attenuation rose at position {position}: {attenuation} above {previous}"
        );
        previous = attenuation;
    }
    let nearest: f64 = coefficients.iter().skip(chain.length()).product();
    let furthest: f64 = coefficients.iter().product();
    assert!(
        furthest < nearest,
        "the prior should be attenuated more than the last transition"
    );
}

#[test]
fn a_group_with_a_member_off_the_chain_refuses_the_contraction_rule() {
    let region = region(Family::Chain, 3, 2);
    let group = vec!["f.prior".to_string(), "f.not_here".to_string()];
    let analysis = InfluenceAnalyzer::default()
        .structural_only()
        .analyse_group(&region, &group, &Perturbation::relative_tolerance(0.1).unwrap())
        .unwrap();
    let refusal = analysis
        .attempted
        .iter()
        .find(|outcome| outcome.method == BoundMethod::ChainContraction)
        .and_then(|outcome| outcome.declined.clone())
        .expect("the contraction rule should have refused");
    assert!(refusal_detail(&refusal).contains("not on the chain"));
}

#[test]
fn the_chain_union_bound_beats_the_multiplicative_one_on_a_mixing_chain() {
    let region = region(Family::Chain, 5, 3);
    let group: Vec<String> = ["f.prior", "f.t0", "f.t1"]
        .iter()
        .map(|id| id.to_string())
        .collect();
    let analysis = InfluenceAnalyzer::default()
        .structural_only()
        .analyse_group(&region, &group, &Perturbation::relative_tolerance(0.2).unwrap())
        .unwrap();
    let composition = analysis
        .attempted
        .iter()
        .find(|outcome| outcome.method == BoundMethod::RatioComposition)
        .and_then(|outcome| outcome.value)
        .expect("the composition rule always applies with a stated range");
    let contraction = analysis
        .attempted
        .iter()
        .find(|outcome| outcome.method == BoundMethod::ChainContraction)
        .and_then(|outcome| outcome.value)
        .expect("the chain rule applies on a chain");
    assert!(
        contraction < composition,
        "propagation bought nothing: {contraction} vs {composition}"
    );
    assert_eq!(
        analysis.estimate.bound().unwrap().method(),
        BoundMethod::ChainContraction
    );
}
