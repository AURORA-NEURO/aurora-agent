//! Invariants of blueprint 25.12, uncertainty and reference standards.

use bioprism_bioir::{
    AdjudicationRecord, CalibrationBin, CalibrationCurve, Representation, ReviewerAssessment,
    ReviewerDistribution, UncertaintyBudget, UncertaintyComponent, UncertaintyError,
    UncertaintyKind,
};
use bioprism_scope::ScopeKey;
use std::collections::BTreeMap;

fn interval(lower: f64, upper: f64) -> Representation {
    Representation::Interval {
        lower,
        upper,
        coverage: 0.95,
    }
}

fn component(kind: UncertaintyKind, representation: Representation) -> UncertaintyComponent {
    UncertaintyComponent::new(kind, representation, "12-month overall survival")
}

#[test]
fn pooling_aleatoric_with_epistemic_uncertainty_is_refused() {
    let aleatoric = component(UncertaintyKind::Aleatoric, interval(0.30, 0.50));
    let epistemic = component(UncertaintyKind::Epistemic, interval(0.20, 0.60));

    assert_eq!(
        UncertaintyBudget::pool(&aleatoric, &epistemic),
        Err(UncertaintyError::CrossKindPooling {
            left: UncertaintyKind::Aleatoric,
            right: UncertaintyKind::Epistemic,
        })
    );
}

#[test]
fn pooling_two_epistemic_intervals_widens_rather_than_averages() {
    let left = component(UncertaintyKind::Epistemic, interval(0.30, 0.50));
    let right = component(UncertaintyKind::Epistemic, interval(0.40, 0.70));

    let pooled = UncertaintyBudget::pool(&left, &right).expect("same kind pools");
    assert_eq!(pooled.kind, UncertaintyKind::Epistemic);
    assert_eq!(pooled.representation, interval(0.30, 0.70));
}

#[test]
fn two_components_of_one_kind_written_differently_are_not_silently_combined() {
    let as_interval = component(UncertaintyKind::Measurement, interval(0.30, 0.50));
    let as_error = component(
        UncertaintyKind::Measurement,
        Representation::StandardError { value: 0.04 },
    );

    assert_eq!(
        UncertaintyBudget::pool(&as_interval, &as_error),
        Err(UncertaintyError::RepresentationsNotCombinable {
            kind: UncertaintyKind::Measurement,
            left: "an interval".to_string(),
            right: "a standard error".to_string(),
        })
    );
}

#[test]
fn a_budget_holds_at_most_one_component_per_kind() {
    let mut budget = UncertaintyBudget::new();
    budget
        .declare(
            "claim-1",
            component(UncertaintyKind::Epistemic, interval(0.30, 0.50)),
        )
        .expect("first component is declared");
    assert_eq!(
        budget.declare(
            "claim-1",
            component(UncertaintyKind::Epistemic, interval(0.10, 0.90)),
        ),
        Err(UncertaintyError::DuplicateKind {
            subject: "claim-1".to_string(),
            kind: UncertaintyKind::Epistemic,
        })
    );
    assert_eq!(budget.len(), 1);
}

#[test]
fn a_decision_requiring_distribution_shift_accounting_fails_on_a_budget_without_it() {
    let mut budget = UncertaintyBudget::new();
    for kind in [
        UncertaintyKind::Aleatoric,
        UncertaintyKind::Epistemic,
        UncertaintyKind::Measurement,
    ] {
        budget
            .declare("claim-1", component(kind, interval(0.30, 0.50)))
            .expect("declares");
    }

    assert_eq!(
        budget.accounts_for("claim-1", &[UncertaintyKind::Aleatoric, UncertaintyKind::Epistemic]),
        Ok(())
    );
    assert_eq!(
        budget.accounts_for("claim-1", &[UncertaintyKind::DistributionShift]),
        Err(UncertaintyError::UnaccountedKind {
            subject: "claim-1".to_string(),
            kind: UncertaintyKind::DistributionShift,
        }),
        "three well-quantified kinds do not stand in for a fourth"
    );
    assert_eq!(budget.kinds().len(), 3);
}

#[test]
fn an_unnormalized_categorical_distribution_is_refused() {
    let mut probabilities = BTreeMap::new();
    probabilities.insert("methylated".to_string(), 0.4);
    probabilities.insert("unmethylated".to_string(), 0.4);

    let mut budget = UncertaintyBudget::new();
    assert!(matches!(
        budget.declare(
            "mgmt-call",
            component(
                UncertaintyKind::Aleatoric,
                Representation::Categorical { probabilities },
            ),
        ),
        Err(UncertaintyError::UnnormalizedDistribution { .. })
    ));
}

#[test]
fn a_probability_outside_the_unit_interval_is_refused() {
    let mut candidates = BTreeMap::new();
    candidates.insert("MONDO:0018177".to_string(), 1.4);
    candidates.insert("MONDO:0005070".to_string(), -0.4);

    let mut budget = UncertaintyBudget::new();
    assert!(matches!(
        budget.declare(
            "mapping-1",
            component(
                UncertaintyKind::Mapping,
                Representation::MappingAmbiguity {
                    source_term: "malignant glioma NOS".to_string(),
                    ontology_version: "mondo-2026-05".to_string(),
                    candidates,
                },
            ),
        ),
        Err(UncertaintyError::ProbabilityOutOfRange { .. })
    ));
}

#[test]
fn an_inverted_interval_and_an_impossible_coverage_are_both_refused() {
    let mut budget = UncertaintyBudget::new();
    assert!(matches!(
        budget.declare("claim-1", component(UncertaintyKind::Epistemic, interval(0.7, 0.2))),
        Err(UncertaintyError::InvertedInterval { .. })
    ));
    assert!(matches!(
        budget.declare(
            "claim-1",
            component(
                UncertaintyKind::Epistemic,
                Representation::Interval {
                    lower: 0.2,
                    upper: 0.7,
                    coverage: 1.5,
                },
            ),
        ),
        Err(UncertaintyError::InvalidCoverage { .. })
    ));
}

fn panel() -> ReviewerDistribution {
    ReviewerDistribution::new(vec![
        ReviewerAssessment::new("reader-1", "progression", true),
        ReviewerAssessment::new("reader-2", "pseudoprogression", true),
        ReviewerAssessment::new("reader-3", "progression", true),
    ])
}

#[test]
fn adjudication_that_drops_a_dissenting_reviewer_is_rejected() {
    let record = AdjudicationRecord {
        adjudicator: "neuro-radiology board".to_string(),
        outcome: "progression".to_string(),
        method: "majority".to_string(),
        dissent: vec![],
    };
    assert_eq!(
        record.validate(&panel()),
        Err(UncertaintyError::DissentErased {
            adjudicator: "neuro-radiology board".to_string(),
            reviewer: "reader-2".to_string(),
        })
    );
}

#[test]
fn adjudication_carrying_every_dissenter_is_accepted() {
    let record = AdjudicationRecord {
        adjudicator: "neuro-radiology board".to_string(),
        outcome: "progression".to_string(),
        method: "majority".to_string(),
        dissent: vec![ReviewerAssessment::new("reader-2", "pseudoprogression", true)],
    };
    assert_eq!(record.validate(&panel()), Ok(()));
}

#[test]
fn reviewer_disagreement_is_reported_as_counts_not_a_majority_label() {
    let distribution = panel();
    let counts = distribution.label_counts();
    assert_eq!(counts.get("progression"), Some(&2));
    assert_eq!(counts.get("pseudoprogression"), Some(&1));
    assert!(!distribution.is_unanimous());
    assert_eq!(distribution.dissenters("progression").len(), 1);
}

#[test]
fn an_empty_reviewer_panel_is_refused() {
    let empty = ReviewerDistribution::new(vec![]);
    assert_eq!(
        empty.validate("read-1"),
        Err(UncertaintyError::NoReviewers {
            subject: "read-1".to_string()
        })
    );
}

#[test]
fn an_ungradable_case_carries_its_reason_instead_of_a_number() {
    let mut budget = UncertaintyBudget::new();
    budget
        .declare(
            "read-1",
            component(
                UncertaintyKind::Expert,
                Representation::Ungradable {
                    reason: "motion artifact obscures the enhancing rim".to_string(),
                },
            ),
        )
        .expect("an ungradable component is a legitimate component");
    assert!(matches!(
        budget.component(UncertaintyKind::Expert).map(|c| &c.representation),
        Some(Representation::Ungradable { .. })
    ));
}

fn curve() -> CalibrationCurve {
    CalibrationCurve {
        label: "12-month survival classifier".to_string(),
        fitted_in: ScopeKey::new().exact("vendor", "siemens"),
        bins: vec![
            CalibrationBin {
                predicted: 0.2,
                observed: 0.3,
                count: 50,
            },
            CalibrationBin {
                predicted: 0.8,
                observed: 0.8,
                count: 50,
            },
        ],
    }
}

#[test]
fn a_calibration_curve_applies_in_a_scope_that_refines_its_own() {
    let query = ScopeKey::new()
        .exact("vendor", "siemens")
        .exact("field_strength", "3T");
    assert_eq!(curve().applies_in(&query), Ok(()));
}

#[test]
fn a_calibration_curve_does_not_apply_outside_the_scope_it_was_fitted_in() {
    let query = ScopeKey::new().exact("vendor", "ge");
    assert!(matches!(
        curve().applies_in(&query),
        Err(UncertaintyError::CalibrationOutOfContext { .. })
    ));
}

#[test]
fn a_scope_incomparable_with_the_fitting_scope_does_not_inherit_the_calibration() {
    let query = ScopeKey::new().exact("field_strength", "3T");
    assert!(
        curve().applies_in(&query).is_err(),
        "unknown is not the same as compatible"
    );
    assert!(
        curve().applies_in(&ScopeKey::new()).is_err(),
        "an unscoped query is broader than the fit, not narrower"
    );
}

#[test]
fn expected_calibration_error_is_a_property_of_one_curve_and_not_of_a_budget() {
    let error = curve().expected_calibration_error().expect("bins are populated");
    assert!((error - 0.05).abs() < 1e-9);

    let empty = CalibrationCurve {
        label: "unfitted".to_string(),
        fitted_in: ScopeKey::new(),
        bins: vec![],
    };
    assert_eq!(empty.expected_calibration_error(), None);
}
