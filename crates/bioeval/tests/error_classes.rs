//! Biological error classes and correctness decomposition (26.02).

mod common;

use bioprism_bioeval::{
    BiologicalErrorClass, ClassifiedError, Conclusion, CorrectnessLayer, Dispersion, LayerError,
    LayerVerdict, LayeredOutcome, Prediction, Severity,
};
use bioprism_section::OracleStatus;
use common::{progression_reference, score, PROGRESSION};

fn assess(
    conclusion: Conclusion,
    observations: impl IntoIterator<Item = (CorrectnessLayer, LayerVerdict)>,
) -> LayeredOutcome {
    LayeredOutcome::assess(conclusion, observations).expect("valid layer observations")
}

fn wrong_conclusion() -> Conclusion {
    Conclusion::Wrong {
        statement: "the lesion is IDH-mutant".to_string(),
    }
}

#[test]
fn a_units_error_and_a_cohort_error_are_not_the_same_failure() {
    let units = BiologicalErrorClass::Units;
    let cohort = BiologicalErrorClass::CohortMismatch;

    assert_ne!(units.severity(), cohort.severity());
    assert!(units.is_mechanically_repairable());
    assert!(
        !cohort.is_mechanically_repairable(),
        "no arithmetic recovers the right cohort from the wrong one"
    );
}

#[test]
fn an_unclassified_error_is_treated_as_critical_rather_than_as_benign() {
    assert_eq!(
        BiologicalErrorClass::Unclassified.severity(),
        Severity::Critical
    );
    assert!(!BiologicalErrorClass::Unclassified
        .severity()
        .admits_partial_credit());
}

#[test]
fn a_benign_class_can_still_be_safety_reaching() {
    let units = BiologicalErrorClass::Units;

    assert_eq!(units.severity(), Severity::Benign);
    assert!(
        units.is_safety_reaching(),
        "the conclusion survives a unit slip; the patient may not"
    );
    assert!(!BiologicalErrorClass::AssociationAsMechanism.is_safety_reaching());
}

#[test]
fn two_runs_with_the_same_wrong_conclusion_are_distinguishable_by_first_failed_layer() {
    let swapped_tube = assess(
        wrong_conclusion(),
        [(
            CorrectnessLayer::SpecimenIdentity,
            LayerVerdict::Failed(ClassifiedError::new(
                CorrectnessLayer::SpecimenIdentity,
                BiologicalErrorClass::SpecimenIdentity,
                "block B-114",
                "block B-141",
            )),
        )],
    );
    let bad_reasoning = assess(
        wrong_conclusion(),
        [
            (CorrectnessLayer::SpecimenIdentity, LayerVerdict::Correct),
            (
                CorrectnessLayer::MechanisticScope,
                LayerVerdict::Failed(ClassifiedError::new(
                    CorrectnessLayer::MechanisticScope,
                    BiologicalErrorClass::AssociationAsMechanism,
                    "co-occurrence read as causation",
                    "association only",
                )),
            ),
        ],
    );

    assert_eq!(bad_reasoning.conclusion(), swapped_tube.conclusion());
    assert_ne!(bad_reasoning.signature(), swapped_tube.signature());
    assert_eq!(
        swapped_tube.first_failed_layer(),
        Some(CorrectnessLayer::SpecimenIdentity)
    );
    assert_eq!(
        bad_reasoning.first_failed_layer(),
        Some(CorrectnessLayer::MechanisticScope)
    );
}

#[test]
fn the_regression_family_groups_by_defect_rather_than_by_conclusion() {
    let outcome = assess(
        wrong_conclusion(),
        [(
            CorrectnessLayer::SpecimenIdentity,
            LayerVerdict::Failed(ClassifiedError::new(
                CorrectnessLayer::SpecimenIdentity,
                BiologicalErrorClass::SpecimenIdentity,
                "block B-114",
                "block B-141",
            )),
        )],
    );

    assert_eq!(
        outcome.signature().regression_family(),
        "specimen_identity/specimen_identity/wrong"
    );
}

#[test]
fn a_layer_downstream_of_a_critical_failure_is_void_rather_than_correct() {
    let outcome = assess(
        wrong_conclusion(),
        [
            (
                CorrectnessLayer::SpecimenIdentity,
                LayerVerdict::Failed(ClassifiedError::new(
                    CorrectnessLayer::SpecimenIdentity,
                    BiologicalErrorClass::SpecimenIdentity,
                    "block B-114",
                    "block B-141",
                )),
            ),
            (CorrectnessLayer::StatisticalEstimand, LayerVerdict::Correct),
        ],
    );

    assert!(outcome.has_void_layers());
    assert_eq!(
        outcome.verdict(CorrectnessLayer::StatisticalEstimand),
        &LayerVerdict::Void {
            blocked_by: CorrectnessLayer::SpecimenIdentity
        },
        "flawless arithmetic about the wrong specimen is not a passing layer"
    );
}

#[test]
fn a_material_failure_leaves_downstream_layers_standing() {
    let outcome = assess(
        wrong_conclusion(),
        [
            (
                CorrectnessLayer::MeasurementInterpretation,
                LayerVerdict::Failed(ClassifiedError::new(
                    CorrectnessLayer::MeasurementInterpretation,
                    BiologicalErrorClass::MagnitudeRightDirection,
                    "2.1-fold",
                    "5.4-fold",
                )),
            ),
            (CorrectnessLayer::DecisionUtility, LayerVerdict::Correct),
        ],
    );

    assert!(!outcome.has_void_layers());
    assert_eq!(
        outcome.verdict(CorrectnessLayer::DecisionUtility),
        &LayerVerdict::Correct,
        "the sign was right, so the decision downstream is still assessable"
    );
    assert_eq!(outcome.worst_severity(), Some(Severity::Benign));
}

#[test]
fn an_unassessed_layer_is_not_a_passing_layer() {
    let outcome = assess(
        Conclusion::Held {
            statement: "no progression".to_string(),
        },
        [(CorrectnessLayer::EntityIdentifier, LayerVerdict::Correct)],
    );

    assert_eq!(
        outcome.verdict(CorrectnessLayer::ScaleTranslation),
        &LayerVerdict::NotAssessed
    );
    assert_eq!(outcome.first_failed_layer(), None);
}

#[test]
fn duplicate_layer_observations_are_refused_instead_of_last_write_wins() {
    let refusal = LayeredOutcome::assess(
        wrong_conclusion(),
        [
            (CorrectnessLayer::EntityIdentifier, LayerVerdict::Correct),
            (
                CorrectnessLayer::EntityIdentifier,
                LayerVerdict::NotAssessed,
            ),
        ],
    )
    .expect_err("one layer must have one recorded observation");

    assert!(matches!(
        refusal,
        LayerError::DuplicateObservation {
            layer: CorrectnessLayer::EntityIdentifier
        }
    ));
}

#[test]
fn caller_supplied_void_verdicts_are_refused_before_propagation() {
    let refusal = LayeredOutcome::assess(
        wrong_conclusion(),
        [(
            CorrectnessLayer::StatisticalEstimand,
            LayerVerdict::Void {
                blocked_by: CorrectnessLayer::SpecimenIdentity,
            },
        )],
    )
    .expect_err("void is a derived state, not an observation");

    assert!(matches!(refusal, LayerError::InvalidVerdict { .. }));
}

#[test]
fn a_classified_error_must_name_the_layer_where_it_was_observed() {
    let refusal = LayeredOutcome::assess(
        wrong_conclusion(),
        [(
            CorrectnessLayer::SpecimenIdentity,
            LayerVerdict::Failed(ClassifiedError::new(
                CorrectnessLayer::MechanisticScope,
                BiologicalErrorClass::SpecimenIdentity,
                "block B-114",
                "block B-141",
            )),
        )],
    )
    .expect_err("a failure cannot be attributed to a different layer");

    assert!(matches!(refusal, LayerError::InvalidVerdict { .. }));
}

#[test]
fn unbounded_conclusion_text_is_refused_before_an_outcome_is_built() {
    let refusal = LayeredOutcome::assess(
        Conclusion::Held {
            statement: "\nnot a bounded statement".to_string(),
        },
        [],
    )
    .expect_err("untrusted conclusion text must not enter a persisted outcome");

    assert!(matches!(refusal, LayerError::InvalidConclusion { .. }));
}

#[test]
fn an_abstention_conclusion_is_not_recorded_as_a_wrong_one() {
    let outcome = assess(
        Conclusion::Withheld {
            reason: "insufficient tissue".to_string(),
        },
        [],
    );

    assert_eq!(outcome.signature().conclusion, "withheld");
    assert!(outcome.errors().next().is_none());
}

#[test]
fn a_critical_error_forces_invalid_even_when_the_reference_is_uncertain() {
    let graded = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    )
    .with_errors([ClassifiedError::new(
        CorrectnessLayer::SpecimenIdentity,
        BiologicalErrorClass::SpecimenIdentity,
        "subject S-07",
        "subject S-70",
    )]);

    assert_eq!(
        graded.status(),
        OracleStatus::Invalid,
        "reference uncertainty must not launder a specimen swap into 'underdetermined'"
    );
    assert!(!graded.is_clean_pass());
    assert!(graded.has_critical_error());
}

#[test]
fn the_error_taxonomy_assigns_every_class_a_severity_without_grader_discretion() {
    for class in BiologicalErrorClass::CANONICAL {
        let severity = class.severity();
        assert_eq!(
            severity,
            class.severity(),
            "severity is a property of the class, read the same way every time"
        );
    }
    assert_eq!(
        BiologicalErrorClass::MolecularSubtype.severity(),
        Severity::Critical
    );
    assert_eq!(
        BiologicalErrorClass::MagnitudeRightDirection.severity(),
        Severity::Benign
    );
}
