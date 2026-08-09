//! An interval that reaches a report cannot be dropped on the way, and one that never existed
//! cannot be invented.

mod common;

use bioprism_metrics::{
    weighted_mean, ClusteringUnit, ConfidenceLevel, Estimate, Interval, IntervalBasis,
    IntervalEstimate, MetricsError, NoIntervalReason, PointEstimate,
};
use common::interval;

#[test]
fn an_interval_estimate_cannot_be_built_around_a_point_it_excludes() {
    let interval = interval(0.10, 0.20, ClusteringUnit::ParentWorld, 30);
    assert!(matches!(
        IntervalEstimate::new(0.90, interval),
        Err(MetricsError::IntervalExcludesEstimate { .. })
    ));
}

#[test]
fn an_interval_estimate_always_serializes_with_its_interval() {
    let estimate = IntervalEstimate::new(0.9, interval(0.85, 0.95, ClusteringUnit::Site, 4))
        .expect("point inside the interval");
    let encoded = serde_json::to_value(&estimate).expect("serializable");
    assert!(encoded.get("interval").is_some());
    assert_eq!(encoded["interval"]["low"], 0.85);
    assert_eq!(encoded["interval"]["basis"]["clustering_unit"], "site");
}

#[test]
fn a_deserialized_interval_estimate_that_excludes_its_own_point_is_refused() {
    let document = r#"{
        "value": 0.9,
        "interval": {
            "low": 0.1,
            "high": 0.2,
            "level": 0.95,
            "basis": {
                "method": "bootstrap",
                "clustering_unit": "parent_world",
                "effective_size": 30
            }
        }
    }"#;
    assert!(serde_json::from_str::<IntervalEstimate>(document).is_err());
}

#[test]
fn a_point_estimate_must_assert_that_no_interval_existed() {
    let estimate = PointEstimate::stated(0.9, NoIntervalReason::SingleTrial).expect("finite value");
    assert_eq!(estimate.no_interval(), NoIntervalReason::SingleTrial);

    for reason in [
        NoIntervalReason::SingleTrial,
        NoIntervalReason::DeterministicQuantity,
        NoIntervalReason::EstimatorNotAvailable,
        NoIntervalReason::ClusteringUnitUnknown,
        NoIntervalReason::WithheldToProtectSmallGroup,
    ] {
        assert!(
            !reason.as_str().contains("drop"),
            "no reason may mean 'an interval existed and was discarded'"
        );
    }
}

#[test]
fn only_a_deterministic_quantity_has_an_intrinsically_absent_interval() {
    assert!(NoIntervalReason::DeterministicQuantity.is_intrinsic());
    for reason in [
        NoIntervalReason::SingleTrial,
        NoIntervalReason::EstimatorNotAvailable,
        NoIntervalReason::ClusteringUnitUnknown,
        NoIntervalReason::WithheldToProtectSmallGroup,
    ] {
        assert!(!reason.is_intrinsic(), "{reason} is evaluation debt");
    }
}

#[test]
fn an_estimate_is_either_an_interval_or_a_stated_absence_and_never_neither() {
    let with = Estimate::with_interval(0.9, interval(0.8, 1.0, ClusteringUnit::ParentWorld, 12))
        .expect("point inside");
    let without = Estimate::point(0.9, NoIntervalReason::SingleTrial).expect("finite");

    assert!(with.interval().is_some() && with.no_interval_reason().is_none());
    assert!(without.interval().is_none() && without.no_interval_reason().is_some());
}

#[test]
fn an_inverted_interval_is_refused() {
    assert!(matches!(
        Interval::new(
            0.9,
            0.1,
            ConfidenceLevel::ninety_five(),
            IntervalBasis::new("bootstrap", ClusteringUnit::ParentWorld, 30),
        ),
        Err(MetricsError::InvertedInterval { .. })
    ));
}

#[test]
fn a_confidence_level_outside_the_open_unit_interval_is_refused() {
    assert!(ConfidenceLevel::new(0.0).is_err());
    assert!(ConfidenceLevel::new(1.0).is_err());
    assert!(ConfidenceLevel::new(f64::NAN).is_err());
    assert!(ConfidenceLevel::new(0.5).is_ok());
}

#[test]
fn adding_intervals_adds_their_widths() {
    let left = interval(0.1, 0.3, ClusteringUnit::ParentWorld, 30);
    let right = interval(0.2, 0.5, ClusteringUnit::ParentWorld, 30);
    let sum = left.add(&right).expect("same level and unit");
    assert!((sum.width() - (left.width() + right.width())).abs() < 1e-12);
}

#[test]
fn subtracting_intervals_widens_rather_than_cancelling() {
    let left = interval(0.4, 0.6, ClusteringUnit::ParentWorld, 30);
    let difference = left.sub(&left).expect("same level and unit");
    assert!(difference.contains(0.0));
    assert!((difference.width() - 2.0 * left.width()).abs() < 1e-12);
    assert!(difference.width() > 0.0);
}

#[test]
fn a_difference_interval_that_excludes_zero_says_so_without_calling_it_significant() {
    let left = interval(0.80, 0.90, ClusteringUnit::ParentWorld, 30);
    let right = interval(0.10, 0.20, ClusteringUnit::ParentWorld, 30);
    let difference = left.sub(&right).expect("same level and unit");
    assert!(difference.excludes(0.0));
    assert!(!difference.contains(0.0));
}

#[test]
fn intervals_at_different_confidence_levels_do_not_combine() {
    let ninety_five = interval(0.1, 0.2, ClusteringUnit::ParentWorld, 30);
    let fifty = Interval::new(
        0.1,
        0.2,
        ConfidenceLevel::new(0.5).expect("valid level"),
        IntervalBasis::new("bootstrap", ClusteringUnit::ParentWorld, 30),
    )
    .expect("well formed");

    assert!(matches!(
        ninety_five.add(&fifty),
        Err(MetricsError::MismatchedConfidenceLevel { .. })
    ));
}

#[test]
fn intervals_clustered_at_different_units_do_not_combine_because_no_ordering_is_defined() {
    let by_parent = interval(0.1, 0.2, ClusteringUnit::ParentWorld, 30);
    let by_site = interval(0.1, 0.2, ClusteringUnit::Site, 4);

    assert!(matches!(
        by_parent.add(&by_site),
        Err(MetricsError::MismatchedClusteringUnit { .. })
    ));
}

#[test]
fn a_combined_interval_rests_on_the_smaller_of_the_two_effective_sizes() {
    let wide = interval(0.1, 0.2, ClusteringUnit::ParentWorld, 300);
    let narrow = interval(0.1, 0.2, ClusteringUnit::ParentWorld, 3);
    let sum = wide.add(&narrow).expect("same level and unit");
    assert_eq!(sum.basis().effective_size, 3);
}

#[test]
fn a_weighted_mean_interval_is_never_narrower_than_the_narrowest_input() {
    let narrow = interval(0.49, 0.51, ClusteringUnit::ParentWorld, 30);
    let wide = interval(0.10, 0.90, ClusteringUnit::ParentWorld, 30);
    let mean =
        weighted_mean(&[(1.0, narrow.clone()), (1.0, wide.clone())]).expect("same level and unit");
    assert!(mean.width() >= narrow.width());
    assert!(mean.width() <= wide.width());
}

#[test]
fn a_weighted_mean_over_no_interval_refuses_rather_than_returning_a_point() {
    assert!(matches!(
        weighted_mean(&[]),
        Err(MetricsError::AggregateOverNothing { .. })
    ));
}

#[test]
fn a_negative_scale_factor_is_refused_because_it_would_invert_the_scoring_direction() {
    let base = interval(0.1, 0.2, ClusteringUnit::ParentWorld, 30);
    assert!(base.scale(-1.0).is_err());
    let doubled = base.scale(2.0).expect("non-negative factor");
    assert!((doubled.width() - 2.0 * base.width()).abs() < 1e-12);
}

#[test]
fn the_trial_is_the_only_clustering_unit_that_is_not_a_dependency_level() {
    assert!(!ClusteringUnit::Trial.is_dependency_level());
    for unit in ClusteringUnit::ALL
        .into_iter()
        .filter(|u| *u != ClusteringUnit::Trial)
    {
        assert!(unit.is_dependency_level(), "{unit} is a dependency level");
    }
}
