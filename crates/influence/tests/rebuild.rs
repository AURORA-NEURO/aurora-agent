//! Perturbing a region must change exactly one thing.
//!
//! Every exact bound is a comparison between two regions. If the rebuild dropped a free variable,
//! a cardinality source or a factor's table, the two regions would answer different questions and
//! the difference between their answers would not be an influence.

use bioprism_backends::{
    CardinalityPolicy, CardinalitySource, DirectMaterialization, QueryBackend, QueryRegion,
    RegionFactor,
};
use bioprism_influence::{perturbed, reference, InfluenceError};

fn fixture() -> QueryRegion {
    QueryRegion::builder("fixture")
        .observed_variable("a", 2)
        .assumed_variable("b", 3)
        .factor(RegionFactor::with_table("f.a", vec!["a"], vec![0.25, 0.75]))
        .factor(RegionFactor::with_table(
            "f.ab",
            vec!["a", "b"],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        ))
        .free("b")
        .assumption("a fixture")
        .build()
        .expect("the fixture is well formed")
}

#[test]
fn a_region_rebuilt_with_the_same_table_answers_identically() {
    let region = fixture();
    let table = region.factors()[0].table().unwrap().to_vec();
    let rebuilt = perturbed::with_replaced_table(&region, "f.a", table, "identity").unwrap();
    let original = DirectMaterialization::new().execute(&region).unwrap();
    let after = DirectMaterialization::new().execute(&rebuilt).unwrap();
    assert!(original.agrees_exactly_with(&after));
}

#[test]
fn rebuilding_preserves_the_free_variables_and_the_semiring() {
    let region = fixture();
    let rebuilt = perturbed::with_factor_removed(&region, "f.a").unwrap();
    assert_eq!(rebuilt.free_variables(), region.free_variables());
    assert_eq!(rebuilt.semiring(), region.semiring());
    assert_eq!(rebuilt.label(), region.label());
    assert_eq!(rebuilt.cardinality(), region.cardinality());
}

#[test]
fn rebuilding_preserves_which_cardinalities_were_assumed_rather_than_observed() {
    let region = fixture();
    let rebuilt = perturbed::with_factor_removed(&region, "f.a").unwrap();
    assert_eq!(
        rebuilt.cardinality_source("b"),
        Some(CardinalitySource::Assumed)
    );
    assert_eq!(
        rebuilt.cardinality_source("a"),
        Some(CardinalitySource::Observed)
    );
    assert_eq!(
        rebuilt.assumed_cardinality_fraction(),
        region.assumed_cardinality_fraction()
    );
}

#[test]
fn a_perturbed_region_records_the_perturbation_in_its_assumptions() {
    let region = fixture();
    let rebuilt = perturbed::with_factor_removed(&region, "f.a").unwrap();
    assert_eq!(rebuilt.assumptions().len(), region.assumptions().len() + 1);
    assert!(rebuilt
        .assumptions()
        .last()
        .unwrap()
        .contains("all-ones potential"));
}

#[test]
fn removing_a_factor_leaves_it_present_with_a_uniform_table_rather_than_deleting_it() {
    let region = fixture();
    let rebuilt = perturbed::with_factor_removed(&region, "f.a").unwrap();
    assert_eq!(rebuilt.factors().len(), region.factors().len());
    let removed = rebuilt
        .factors()
        .iter()
        .find(|factor| factor.id() == "f.a")
        .unwrap();
    assert_eq!(removed.table(), Some([1.0, 1.0].as_slice()));
    assert_eq!(removed.scope(), region.factors()[0].scope());
}

#[test]
fn replacing_a_table_of_the_wrong_size_is_rejected_rather_than_truncated() {
    let region = fixture();
    let error =
        perturbed::with_replaced_table(&region, "f.a", vec![1.0, 1.0, 1.0], "wrong size").unwrap_err();
    assert!(matches!(
        error,
        InfluenceError::PerturbedRegionRejected { .. }
    ));
}

#[test]
fn perturbing_an_absent_factor_is_a_caller_bug_not_an_unknown_influence() {
    let region = fixture();
    let error = perturbed::with_factor_removed(&region, "f.nope").unwrap_err();
    assert!(matches!(
        error,
        InfluenceError::UnknownFactor { .. }
    ));
}

#[test]
fn perturbing_an_untabled_factor_is_rejected_before_any_arithmetic() {
    let world = reference::reference_world().unwrap();
    let region = QueryRegion::from_world_slice(
        &world,
        "structural",
        [reference::REFERENCE_TARGET],
        &CardinalityPolicy::default(),
    )
    .unwrap();
    let id = region.factors()[0].id().to_string();
    let error = perturbed::with_factor_removed(&region, &id).unwrap_err();
    assert!(matches!(error, InfluenceError::UntabledFactor { .. }));
}

#[test]
fn an_answer_with_zero_total_mass_has_no_normalised_form() {
    let region = QueryRegion::builder("annihilated")
        .observed_variable("a", 2)
        .factor(RegionFactor::with_table("f.a", vec!["a"], vec![0.0, 0.0]))
        .free("a")
        .build()
        .unwrap();
    let computed = DirectMaterialization::new().execute(&region).unwrap();
    let error = bioprism_influence::AnswerDistribution::normalise(&computed).unwrap_err();
    assert!(matches!(error, InfluenceError::DegenerateAnswer { .. }));
}

#[test]
fn answers_over_different_scopes_are_incomparable_rather_than_far_apart() {
    let left = bioprism_influence::AnswerDistribution::from_parts(
        vec!["a".to_string()],
        vec![0.5, 0.5],
    )
    .unwrap();
    let right = bioprism_influence::AnswerDistribution::from_parts(
        vec!["b".to_string()],
        vec![0.5, 0.5],
    )
    .unwrap();
    let error = bioprism_influence::total_variation(&left, &right).unwrap_err();
    assert!(matches!(error, InfluenceError::IncomparableScopes { .. }));
}

#[test]
fn total_variation_of_a_distribution_with_itself_is_zero_and_with_a_disjoint_one_is_one() {
    let scope = vec!["a".to_string()];
    let left =
        bioprism_influence::AnswerDistribution::from_parts(scope.clone(), vec![1.0, 0.0]).unwrap();
    let right = bioprism_influence::AnswerDistribution::from_parts(scope, vec![0.0, 1.0]).unwrap();
    assert_eq!(
        bioprism_influence::total_variation(&left, &left).unwrap(),
        0.0
    );
    assert_eq!(
        bioprism_influence::total_variation(&left, &right).unwrap(),
        1.0
    );
}

#[test]
fn a_relative_tolerance_at_or_above_one_is_rejected() {
    assert!(bioprism_influence::Perturbation::relative_tolerance(1.0).is_err());
    assert!(bioprism_influence::Perturbation::relative_tolerance(-0.1).is_err());
    assert!(bioprism_influence::Perturbation::relative_tolerance(0.999).is_ok());
}
