//! The shared provenance currency: what an answer rests on, and over how much of the population.

use bioprism_dataops::{Attested, Basis, BasisError, Coverage, Epoch, PartyId};

fn party(name: &str) -> PartyId {
    PartyId::parse(name).expect("a plain name is a valid party id")
}

#[test]
fn two_answers_with_the_same_value_and_different_bases_are_not_equal() {
    let measured = Attested::first_hand(42u64, Epoch::new(3));
    let claimed = Attested::declared(42u64, party("worker-1"), Epoch::new(3));

    assert_eq!(measured.value(), claimed.value());
    assert_ne!(measured, claimed);
}

#[test]
fn the_same_value_at_different_coverages_is_a_different_answer() {
    let complete = Attested::new(
        99u64,
        Basis::FirstHand {
            observed_at: Epoch::new(1),
        },
        Coverage::Complete { observed: 100 },
    );
    let partial = Attested::new(
        99u64,
        Basis::FirstHand {
            observed_at: Epoch::new(1),
        },
        Coverage::Partial {
            observed: 10,
            expected: 100,
        },
    );

    assert_ne!(complete, partial);
}

#[test]
fn an_unobserved_basis_has_no_observation_epoch_and_no_age() {
    let basis = Basis::Unobserved {
        reason: "the probe was skipped".to_string(),
    };

    assert_eq!(basis.observed_at(), None);
    assert_eq!(basis.age_at(Epoch::new(9)), None);
    assert!(!basis.is_first_hand());
}

#[test]
fn a_derived_basis_reports_lag_without_claiming_an_observation_epoch() {
    let basis = Basis::Derived {
        source: "index".to_string(),
        lag_epochs: 4,
    };

    assert_eq!(basis.observed_at(), None);
    assert_eq!(basis.name(), "derived");
}

#[test]
fn the_weakest_basis_of_nothing_is_unobserved_rather_than_first_hand() {
    let weakest = Basis::weakest_of([]);

    assert!(matches!(weakest, Basis::Unobserved { .. }));
}

#[test]
fn a_declaration_beside_a_measurement_yields_the_declaration() {
    let measured = Basis::FirstHand {
        observed_at: Epoch::new(2),
    };
    let claimed = Basis::Declared {
        by: party("provider-a"),
        declared_at: Epoch::new(2),
    };

    assert_eq!(Basis::weakest_of([&measured, &claimed]), claimed);
}

#[test]
fn an_unobserved_component_dominates_every_other_basis() {
    let measured = Basis::FirstHand {
        observed_at: Epoch::new(2),
    };
    let missing = Basis::Unobserved {
        reason: "nobody checked the region".to_string(),
    };

    assert_eq!(Basis::weakest_of([&measured, &missing]), missing);
}

#[test]
fn coverage_refuses_more_observations_than_the_population_admits() {
    let error = Coverage::of(11, 10).expect_err("eleven of ten is a counting error");

    assert_eq!(
        error,
        BasisError::CoverageExceedsPopulation {
            observed: 11,
            expected: 10
        }
    );
}

#[test]
fn coverage_over_an_unknown_population_offers_no_denominator_to_divide_by() {
    let coverage = Coverage::NoDenominator {
        observed: 400,
        reason: "telemetry gap".to_string(),
    };

    assert_eq!(coverage.observed(), 400);
    assert_eq!(coverage.expected(), None);
    assert!(!coverage.is_complete());
}

#[test]
fn a_complete_coverage_is_distinct_from_a_partial_one_that_saw_the_same_count() {
    let complete = Coverage::of(10, 10).expect("ten of ten");
    let partial = Coverage::of(10, 20).expect("ten of twenty");

    assert!(complete.is_complete());
    assert!(!partial.is_complete());
    assert_ne!(complete, partial);
}

#[test]
fn combined_coverage_loses_its_denominator_as_soon_as_one_component_has_none() {
    let known = Coverage::of(5, 5).expect("five of five");
    let unknown = Coverage::NoDenominator {
        observed: 3,
        reason: "collector restarted".to_string(),
    };

    let combined = Coverage::weakest_of([&known, &unknown]);

    assert_eq!(combined.expected(), None);
    assert_eq!(combined.observed(), 8);
}

#[test]
fn combined_coverage_of_complete_parts_is_complete() {
    let left = Coverage::of(4, 4).expect("four of four");
    let right = Coverage::of(6, 6).expect("six of six");

    assert_eq!(
        Coverage::weakest_of([&left, &right]),
        Coverage::Complete { observed: 10 }
    );
}

#[test]
fn mapping_an_attested_value_cannot_strengthen_its_basis() {
    let claimed = Attested::declared(7u64, party("worker-1"), Epoch::new(5));

    let doubled = claimed.map(|value| value * 2);

    assert_eq!(*doubled.value(), 14);
    assert!(!doubled.basis().is_first_hand());
}

#[test]
fn a_party_id_rejects_a_control_character() {
    let error = PartyId::parse("hub\u{7}one").expect_err("a bell character is not a name");

    assert!(matches!(error, BasisError::MalformedField { .. }));
}

#[test]
fn a_serialised_basis_keeps_the_variant_that_distinguishes_it() {
    let claimed = Basis::Declared {
        by: party("provider-a"),
        declared_at: Epoch::new(1),
    };

    let json = serde_json::to_string(&claimed).expect("basis serialises");
    let back: Basis = serde_json::from_str(&json).expect("basis round-trips");

    assert!(json.contains("declared"));
    assert_eq!(back, claimed);
    assert_ne!(
        back,
        Basis::FirstHand {
            observed_at: Epoch::new(1)
        }
    );
}
