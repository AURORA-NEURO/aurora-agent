//! The worked example against the shipped reference world, and the finding that comes out of it.
//!
//! `AGENTS.md`: "If a measurement disagrees with the thesis, that is the measurement we publish."
//! These tests pin the number that disagrees.

use bioprism_influence::{
    manifest, reference, structural_zero, InfluenceAnalyzer, InfluenceEstimate, Perturbation,
    UnknownReason,
};
use bioprism_section::InfluenceClass;

#[test]
fn the_reference_world_slice_reaches_six_factors_over_seventeen_variables() {
    let measurement = reference::measure().expect("the shipped fixture parses");
    assert_eq!(measurement.world_id, "radiogenomic-integrity-demo-v1");
    assert_eq!(measurement.total_facts, 761);
    assert_eq!(measurement.total_factors, 756);
    assert_eq!(measurement.region_factors, 6);
    assert_eq!(measurement.region_variables, 17);
    assert_eq!(measurement.unreached_factors, 750);
}

#[test]
fn no_reference_world_group_moves_from_unknown_to_bounded_under_removal() {
    let measurement = reference::measure().expect("the shipped fixture parses");
    assert_eq!(
        measurement.bounded_under_removal(),
        0,
        "the headline finding changed: {}",
        measurement.headline()
    );
}

#[test]
fn every_unbounded_reference_factor_names_the_schema_gap_rather_than_a_number() {
    let measurement = reference::measure().expect("the shipped fixture parses");
    for finding in &measurement.removal {
        match &finding.estimate {
            InfluenceEstimate::Unknown(UnknownReason::NoFactorTable { factor }) => {
                assert_eq!(factor, &finding.factor);
            }
            other => panic!("{}: expected the schema gap, got {other:?}", finding.factor),
        }
        assert!(manifest::certificate_bound(&finding.estimate).is_none());
    }
}

#[test]
fn a_declared_range_bounds_every_reference_factor_without_executing_anything() {
    let measurement = reference::measure().expect("the shipped fixture parses");
    assert_eq!(measurement.bounded_under_hypothetical_range(), 6);
    let bound = measurement
        .hypothetical_bound()
        .expect("a stated range always bounds");
    assert!((bound - 0.050_125_628_933_800_51).abs() < 1e-12, "bound was {bound}");
    for finding in &measurement.hypothetical_stated_range {
        assert_eq!(
            finding.bounded_at(),
            Some(bound),
            "{}: the path-blind bound should not vary with the factor",
            finding.factor
        );
    }
}

#[test]
fn the_uniform_valuation_bounds_everything_at_zero_and_that_is_not_a_result() {
    let measurement = reference::measure().expect("the shipped fixture parses");
    for finding in &measurement.uniform_valuation {
        assert_eq!(finding.bounded_at(), Some(0.0));
    }

    let world = reference::reference_world().unwrap();
    let region = reference::reference_region(&world)
        .unwrap()
        .with_uniform_tables()
        .unwrap();
    assert!(
        region
            .assumptions()
            .iter()
            .any(|note| note.contains("carry no evidential content")),
        "the fabricated valuation must be declared on the region"
    );
}

#[test]
fn the_seven_hundred_and_fifty_unreached_factors_are_structurally_zero_not_bounded() {
    let measurement = reference::measure().expect("the shipped fixture parses");
    let group = manifest::omission_group(
        "no backward dependency path to any target under the declared factor graph",
        measurement.unreached_factors,
        &InfluenceEstimate::Bounded(structural_zero("no path reaches the target").unwrap()),
        Vec::new(),
    );
    assert_eq!(group.influence, InfluenceClass::Zero);
    assert_eq!(group.count, 750);
    assert_eq!(group.bound, Some(0.0));
    assert!(!manifest::is_informative(&group));
}

#[test]
fn the_reference_measurement_is_deterministic_across_runs() {
    let first = reference::measure().unwrap();
    let second = reference::measure().unwrap();
    assert_eq!(first, second);
}

#[test]
fn the_reference_region_carries_no_potentials_at_all() {
    let world = reference::reference_world().unwrap();
    let region = reference::reference_region(&world).unwrap();
    assert!(!region.has_tables());
    for factor in region.factors() {
        assert!(
            factor.table().is_none(),
            "{} unexpectedly carries a potential",
            factor.id()
        );
    }
}

#[test]
fn a_manifest_built_from_the_reference_world_does_not_support_a_sufficiency_claim() {
    let measurement = reference::measure().expect("the shipped fixture parses");
    let mut groups = vec![manifest::omission_group(
        "no backward dependency path to any target",
        measurement.unreached_factors,
        &InfluenceEstimate::Bounded(structural_zero("no path reaches the target").unwrap()),
        Vec::new(),
    )];
    for finding in &measurement.removal {
        groups.push(manifest::omission_group(
            format!("omitted evidence governed by {}", finding.factor),
            1,
            &finding.estimate,
            Vec::new(),
        ));
    }
    let summary = manifest::summarise(&groups);
    assert_eq!(summary.unknown_groups, 6);
    assert_eq!(summary.bounded_groups, 0);
    assert_eq!(summary.informative_groups, 0);
    assert_eq!(summary.worst_informative_bound, None);
}

#[test]
fn the_headline_states_the_finding_rather_than_burying_it() {
    let measurement = reference::measure().unwrap();
    let headline = measurement.headline();
    assert!(headline.contains("0 of 6 region factors are bounded under removal"));
    assert!(headline.contains("carry no potential"));
}

#[test]
fn the_analyzer_declines_the_reference_region_the_same_way_whether_or_not_it_may_execute() {
    let world = reference::reference_world().unwrap();
    let region = reference::reference_region(&world).unwrap();
    let executing = InfluenceAnalyzer::default();
    let structural = InfluenceAnalyzer::default().structural_only();
    for factor in region.factors() {
        let with = executing
            .analyse_factor(&region, factor.id(), &Perturbation::Removal)
            .unwrap();
        let without = structural
            .analyse_factor(&region, factor.id(), &Perturbation::Removal)
            .unwrap();
        assert_eq!(with.estimate, without.estimate);
        assert!(with.attempted.len() > without.attempted.len());
    }
}
