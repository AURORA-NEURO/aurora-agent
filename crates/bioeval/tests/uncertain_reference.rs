//! Scoring against a reference standard that is itself uncertain (31.01).

mod common;

use bioprism_bioeval::{
    CollapseError, CollapsePolicy, Dispersion, Prediction, ReferenceDischarge,
    ReferenceDistribution, ReferenceError, ReferenceStandard, ScoreError,
};
use bioprism_section::OracleStatus;
use common::{
    calibrated_forecast, grader, progression_reference, resolved_reference, score,
    sixty_percent_reference, witness, MIXED, PROGRESSION, TREATMENT_EFFECT,
};

const EPS: f64 = 1e-9;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6
}

fn aleatoric_policy() -> CollapsePolicy {
    CollapsePolicy::strict("bioeval/aleatoric/1")
}

fn annotation_policy() -> CollapsePolicy {
    CollapsePolicy {
        policy_id: "bioeval/annotation/1".to_string(),
        discharge: ReferenceDischarge::AnnotationError,
        minimum_reference_confidence: 0.5,
        require_attributed_dispersion: true,
    }
}

#[test]
fn agreeing_with_an_uncertain_reference_does_not_score_as_a_clean_pass() {
    let graded = score(
        &Prediction::categorical(PROGRESSION),
        &sixty_percent_reference(),
    );

    assert!(
        !graded.is_clean_pass(),
        "the prediction named the reference's own modal answer, and the reference is only 60% \
         confident in it"
    );
    assert!(graded.interval().width() > EPS);
    assert!(close(graded.interval().under_aleatoric, 0.84));
    assert!(close(graded.interval().under_annotation_error, 1.0));
}

#[test]
fn agreeing_with_a_resolved_reference_does_score_as_a_clean_pass() {
    let graded = score(
        &Prediction::categorical(PROGRESSION),
        &resolved_reference(PROGRESSION),
    );

    assert!(graded.is_clean_pass());
    assert!(graded.interval().is_point());
    assert!(close(graded.interval().lo(), 1.0));
    assert_eq!(graded.status(), OracleStatus::Valid);
}

#[test]
fn an_uncertain_reference_leaves_the_score_a_band_and_a_resolved_one_leaves_a_point() {
    let uncertain = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );
    let resolved = score(
        &Prediction::categorical(PROGRESSION),
        &resolved_reference(PROGRESSION),
    );

    assert!(!uncertain.interval().is_point());
    assert!(resolved.interval().is_point());
    assert!(uncertain.reference_entropy_bits() > resolved.reference_entropy_bits());
}

#[test]
fn a_wrong_answer_against_a_resolved_reference_is_a_point_at_zero_not_a_band() {
    let reference = ReferenceStandard::Distribution(
        ReferenceDistribution::new(
            [
                (PROGRESSION.to_string(), 1.0),
                (TREATMENT_EFFECT.to_string(), 0.0),
            ],
            Dispersion::Aleatoric,
        )
        .expect("masses sum to one"),
    );

    let graded = score(&Prediction::categorical(TREATMENT_EFFECT), &reference);

    assert!(graded.interval().is_point());
    assert!(close(graded.interval().lo(), 0.0));
    assert_eq!(graded.status(), OracleStatus::Invalid);
    assert!(!graded.is_clean_pass());
}

#[test]
fn a_calibrated_forecast_outscores_an_unqualified_categorical_label() {
    let reference = progression_reference(Dispersion::Aleatoric);
    let policy = aleatoric_policy();

    let hedged = score(&calibrated_forecast(), &reference)
        .collapse(&policy)
        .expect("an attributed aleatoric reference collapses under an aleatoric policy");
    let flat = score(&Prediction::categorical(PROGRESSION), &reference)
        .collapse(&policy)
        .expect("an attributed aleatoric reference collapses under an aleatoric policy");

    assert!(close(hedged, 1.0), "the forecast matched the reference");
    assert!(close(flat, 0.8325));
    assert!(
        hedged > flat,
        "31.01's worked case: the calibrated forecast is more correct than the label"
    );
}

#[test]
fn the_ordering_reverses_when_the_spread_is_annotation_error_rather_than_biology() {
    let reference = progression_reference(Dispersion::AnnotationError);
    let policy = annotation_policy();

    let hedged = score(&calibrated_forecast(), &reference)
        .collapse(&policy)
        .expect("an annotation-error reference admits the annotation-error discharge");
    let flat = score(&Prediction::categorical(PROGRESSION), &reference)
        .collapse(&policy)
        .expect("an annotation-error reference admits the annotation-error discharge");

    assert!(close(flat, 1.0));
    assert!(close(hedged, 0.8325));
    assert!(
        flat > hedged,
        "if the readers only disagreed because the rubric was vague, the confident answer was right"
    );
}

#[test]
fn a_policy_that_reads_biological_spread_as_noise_is_refused() {
    let graded = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );

    let refusal = graded
        .collapse(&annotation_policy())
        .expect_err("the reference declared its spread irreducible");

    assert!(matches!(
        refusal,
        CollapseError::DischargeContradictsDispersion { .. }
    ));
}

#[test]
fn no_scalar_can_be_obtained_from_an_unattributed_reference() {
    let graded = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Unattributed),
    );

    for discharge in [
        ReferenceDischarge::Aleatoric,
        ReferenceDischarge::AnnotationError,
        ReferenceDischarge::AsDeclared,
    ] {
        let policy = CollapsePolicy {
            policy_id: "bioeval/probe/1".to_string(),
            discharge,
            minimum_reference_confidence: 0.0,
            require_attributed_dispersion: false,
        };
        assert!(
            matches!(
                graded.collapse(&policy),
                Err(CollapseError::UnattributedDispersion { .. })
            ),
            "discharge {discharge:?} must not produce a number from unattributed spread"
        );
    }
}

#[test]
fn a_conservative_discharge_is_the_one_route_through_an_unattributed_reference() {
    let graded = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Unattributed),
    );

    let permissive = CollapsePolicy {
        policy_id: "bioeval/conservative/1".to_string(),
        discharge: ReferenceDischarge::Conservative,
        minimum_reference_confidence: 0.0,
        require_attributed_dispersion: false,
    };
    let value = graded
        .collapse(&permissive)
        .expect("the pessimistic end cannot overstate, so it is admissible");
    assert!(close(value, graded.interval().lo()));

    let strict = CollapsePolicy {
        require_attributed_dispersion: true,
        ..permissive
    };
    assert!(
        matches!(
            graded.collapse(&strict),
            Err(CollapseError::UnattributedDispersion { .. })
        ),
        "a policy may still forbid publishing a lower bound derived from unexamined spread"
    );
}

#[test]
fn a_declared_mixed_dispersion_interpolates_between_the_two_readings() {
    let graded = score(
        &calibrated_forecast(),
        &progression_reference(Dispersion::Mixed {
            aleatoric_fraction: 0.5,
        }),
    );

    let policy = CollapsePolicy {
        policy_id: "bioeval/as-declared/1".to_string(),
        discharge: ReferenceDischarge::AsDeclared,
        minimum_reference_confidence: 0.5,
        require_attributed_dispersion: true,
    };
    let value = graded.collapse(&policy).expect("mixed is attributed");

    assert!(close(value, (1.0 + 0.8325) / 2.0));
}

#[test]
fn a_reference_below_the_policy_confidence_floor_is_not_scored() {
    let diffuse = ReferenceStandard::Distribution(
        ReferenceDistribution::new(
            [
                (PROGRESSION.to_string(), 0.4),
                (TREATMENT_EFFECT.to_string(), 0.35),
                (MIXED.to_string(), 0.25),
            ],
            Dispersion::Aleatoric,
        )
        .expect("masses sum to one"),
    );

    let graded = score(&Prediction::categorical(PROGRESSION), &diffuse);
    let refusal = graded
        .collapse(&aleatoric_policy())
        .expect_err("0.4 modal confidence is below the 0.5 floor");

    assert!(matches!(
        refusal,
        CollapseError::ReferenceBelowPolicyFloor { .. }
    ));
}

#[test]
fn an_unresolved_reference_refuses_to_score_rather_than_returning_zero() {
    let reference = ReferenceStandard::Unresolved {
        reason: "the adjudication panel deadlocked".to_string(),
    };

    let refusal = grader()
        .grade(
            &witness(),
            "case-1",
            &Prediction::categorical(PROGRESSION),
            &reference,
        )
        .expect_err("an unresolved reference is not a zero");

    assert!(matches!(refusal, ScoreError::ReferenceUnresolved { .. }));
}

#[test]
fn a_not_evaluable_reference_is_distinguishable_from_an_unresolved_one() {
    let out_of_scope = ReferenceStandard::NotEvaluable {
        reason: "paediatric case, panel is adult-only".to_string(),
    };

    let refusal = grader()
        .grade(
            &witness(),
            "case-1",
            &Prediction::categorical(PROGRESSION),
            &out_of_scope,
        )
        .expect_err("out of scope is not a score");

    assert!(matches!(refusal, ScoreError::ReferenceNotEvaluable { .. }));
    assert!(!out_of_scope.can_certify_a_clean_pass());
}

#[test]
fn abstention_is_graded_as_abstention_not_as_a_wrong_answer() {
    let grade = grader()
        .grade(
            &witness(),
            "case-1",
            &Prediction::Abstained {
                reason: "conflicting perfusion and diffusion signal".to_string(),
            },
            &progression_reference(Dispersion::Aleatoric),
        )
        .expect("abstention is gradeable");

    assert!(
        grade.score().is_none(),
        "there is no numeric channel to sink into"
    );
    let record = grade.abstention().expect("an abstention record exists");
    assert!(
        record.warranted,
        "the reference carries over a bit of entropy, so declining was defensible"
    );
}

/// The threshold builder is the only writer of the bar `grade_abstention` reads.
///
/// One abstention against one reference, graded twice. Nothing about the prediction or the
/// reference changes between the two grades, so a flipped `warranted` can only have come from the
/// grader's own threshold — which is the property that would be lost if the builder were removed
/// and every grader shared the one-bit default.
#[test]
fn the_same_abstention_is_warranted_or_not_according_to_the_graders_threshold() {
    let abstention = Prediction::Abstained {
        reason: "conflicting perfusion and diffusion signal".to_string(),
    };
    let reference = progression_reference(Dispersion::Aleatoric);

    let grade_at_default = grader()
        .grade(&witness(), "case-1", &abstention, &reference)
        .expect("abstention is gradeable");
    let grade_at_two_bits = grader()
        .with_abstention_threshold(2.0)
        .grade(&witness(), "case-1", &abstention, &reference)
        .expect("abstention is gradeable");

    assert!(
        grade_at_default
            .abstention()
            .expect("an abstention record exists")
            .warranted
    );
    assert!(
        !grade_at_two_bits
            .abstention()
            .expect("an abstention record exists")
            .warranted
    );
}

#[test]
fn abstention_against_a_reference_that_decided_is_recorded_as_unwarranted() {
    let grade = grader()
        .grade(
            &witness(),
            "case-1",
            &Prediction::Abstained {
                reason: "unsure".to_string(),
            },
            &resolved_reference(PROGRESSION),
        )
        .expect("abstention is gradeable");

    let record = grade.abstention().expect("an abstention record exists");
    assert!(!record.warranted);
    assert_eq!(record.reference_modal_confidence, Some(1.0));
}

#[test]
fn an_uncertain_reference_projects_to_underdetermined_not_valid() {
    let graded = score(
        &Prediction::categorical(PROGRESSION),
        &sixty_percent_reference(),
    );

    assert_eq!(
        graded.status(),
        OracleStatus::Underdetermined,
        "a three-valued consumer must not be handed a pass for a case the reference could not decide"
    );
}

#[test]
fn a_prediction_outside_the_reference_state_space_is_refused_not_scored_zero() {
    let refusal = grader()
        .grade(
            &witness(),
            "case-1",
            &Prediction::categorical("pseudoprogression"),
            &progression_reference(Dispersion::Aleatoric),
        )
        .expect_err("the reference never enumerated that state");

    assert!(matches!(
        refusal,
        ScoreError::StateOutsideReference { state } if state == "pseudoprogression"
    ));
}

#[test]
fn reference_masses_that_do_not_sum_to_one_are_rejected_rather_than_renormalised() {
    let refusal = ReferenceDistribution::new(
        [
            (PROGRESSION.to_string(), 0.5),
            (TREATMENT_EFFECT.to_string(), 0.4),
        ],
        Dispersion::Aleatoric,
    )
    .expect_err("0.9 is not a distribution");

    assert!(matches!(refusal, ReferenceError::MassNotNormalised { .. }));
}

#[test]
fn a_tied_reference_reports_the_tie_rather_than_picking_a_side() {
    let tied = ReferenceDistribution::new(
        [
            (PROGRESSION.to_string(), 0.5),
            (TREATMENT_EFFECT.to_string(), 0.5),
        ],
        Dispersion::Aleatoric,
    )
    .expect("masses sum to one");

    assert!(tied.is_modally_tied());
    assert!(close(tied.modal_confidence(), 0.5));
}

#[test]
fn a_score_round_trips_through_json_without_losing_the_uncertainty() {
    let graded = score(
        &Prediction::categorical(PROGRESSION),
        &sixty_percent_reference(),
    );

    let encoded = serde_json::to_string(&graded).expect("a score serialises");
    let decoded: bioprism_bioeval::BioScore =
        serde_json::from_str(&encoded).expect("a score deserialises");

    assert_eq!(decoded, graded);
    assert!(!decoded.is_clean_pass());
    assert_eq!(decoded.status(), OracleStatus::Underdetermined);
}
