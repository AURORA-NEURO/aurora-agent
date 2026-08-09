//! Comparability gates: refusing a number rather than producing a meaningless one (26.10).

mod common;

use bioprism_bioeval::{
    gate, Bridge, ComparabilityRequirement, Dispersion, FrameDimension, FrameSide, Grader,
    Incomparability, MeasurementFrame, Prediction, ScoreError,
};
use bioprism_scope::ScopeClass;
use common::{declared_frame, progression_reference, witness, PROGRESSION, REQUIREMENT_ID};

#[test]
fn two_frames_that_both_leave_a_dimension_undeclared_are_not_comparable() {
    let silent = declared_frame();
    let mut half_silent = MeasurementFrame::default();
    for dimension in FrameDimension::CANONICAL {
        if dimension != FrameDimension::CoordinateFrame {
            half_silent = half_silent.with(dimension, silent.get(dimension).unwrap_or("x"));
        }
    }
    let other = half_silent.clone();

    let failures = gate(
        &ComparabilityRequirement::strict("r"),
        &half_silent,
        &other,
        &[],
    )
    .expect_err("mutual silence about the coordinate frame is not agreement about it");

    assert_eq!(failures.len(), 1);
    assert!(matches!(
        failures[0],
        Incomparability::Undeclared {
            dimension: FrameDimension::CoordinateFrame,
            side: FrameSide::Both
        }
    ));
}

#[test]
fn a_comparison_across_reference_builds_is_refused_rather_than_scored() {
    let prediction = declared_frame().with(FrameDimension::ReferenceBuild, "GRCh37");
    let reference = declared_frame().with(FrameDimension::ReferenceBuild, "GRCh38");

    let failures = gate(
        &ComparabilityRequirement::strict("r"),
        &prediction,
        &reference,
        &[],
    )
    .expect_err("the same coordinate names different loci in the two builds");

    assert!(matches!(
        &failures[0],
        Incomparability::ValueDiffers { dimension, prediction, reference }
            if *dimension == FrameDimension::ReferenceBuild
                && prediction == "GRCh37"
                && reference == "GRCh38"
    ));
}

#[test]
fn every_failing_dimension_is_reported_not_only_the_first() {
    let prediction = declared_frame()
        .with(FrameDimension::ReferenceBuild, "GRCh37")
        .with(FrameDimension::Unit, "cm")
        .with(FrameDimension::Normalisation, "z_score");
    let reference = declared_frame();

    let failures = gate(
        &ComparabilityRequirement::strict("r"),
        &prediction,
        &reference,
        &[],
    )
    .expect_err("three dimensions disagree");

    let dimensions: Vec<FrameDimension> = failures.iter().map(Incomparability::dimension).collect();
    assert_eq!(dimensions.len(), 3);
    assert!(dimensions.contains(&FrameDimension::ReferenceBuild));
    assert!(dimensions.contains(&FrameDimension::Unit));
    assert!(dimensions.contains(&FrameDimension::Normalisation));
}

#[test]
fn a_declared_bridge_reconciles_a_dimension_and_records_that_it_was_used() {
    let prediction = declared_frame().with(FrameDimension::ReferenceBuild, "GRCh37");
    let reference = declared_frame().with(FrameDimension::ReferenceBuild, "GRCh38");
    let liftover = Bridge {
        bridge_id: "ucsc/hg19ToHg38".to_string(),
        dimension: FrameDimension::ReferenceBuild,
        from: "GRCh37".to_string(),
        to: "GRCh38".to_string(),
        loss: 0.0,
    };

    let earned = gate(
        &ComparabilityRequirement::strict("r"),
        &prediction,
        &reference,
        &[liftover],
    )
    .expect("a declared lossless bridge reconciles the builds");

    assert!(!earned.is_direct(), "the comparison went through a bridge");
    assert_eq!(earned.bridges()[0].bridge_id, "ucsc/hg19ToHg38");
    assert_eq!(
        earned.reconciled(FrameDimension::ReferenceBuild),
        Some("GRCh38")
    );
}

#[test]
fn a_bridge_beyond_the_requirements_tolerance_is_refused() {
    let prediction = declared_frame().with(FrameDimension::AssayPlatform, "illumina_450k");
    let reference = declared_frame().with(FrameDimension::AssayPlatform, "illumina_epic");
    let harmonisation = Bridge {
        bridge_id: "harmonise/450k-epic".to_string(),
        dimension: FrameDimension::AssayPlatform,
        from: "illumina_450k".to_string(),
        to: "illumina_epic".to_string(),
        loss: 0.08,
    };

    let failures = gate(
        &ComparabilityRequirement::strict("r"),
        &prediction,
        &reference,
        std::slice::from_ref(&harmonisation),
    )
    .expect_err("the strict requirement tolerates no loss");
    assert!(matches!(
        failures[0],
        Incomparability::BridgeTooLossy { loss, .. } if loss == 0.08
    ));

    let tolerant = ComparabilityRequirement::strict("r").tolerating_loss(0.1);
    let earned = gate(&tolerant, &prediction, &reference, &[harmonisation])
        .expect("a requirement that names its tolerance admits the bridge");
    assert!((earned.total_bridge_loss() - 0.08).abs() < 1e-12);
}

#[test]
fn a_witness_earned_under_a_laxer_requirement_is_rejected_by_a_stricter_grader() {
    let lax = ComparabilityRequirement::over("bioeval/lax/1", [FrameDimension::Unit]);
    let lax_witness = gate(&lax, &declared_frame(), &declared_frame(), &[])
        .expect("one matching dimension satisfies the lax gate");

    let strict_grader = Grader::new("strict", ComparabilityRequirement::strict(REQUIREMENT_ID));
    let refusal = strict_grader
        .grade(
            &lax_witness,
            "case-1",
            &Prediction::categorical(PROGRESSION),
            &progression_reference(Dispersion::Aleatoric),
        )
        .expect_err("the gate is not bypassable by bringing your own requirement");

    assert!(matches!(
        refusal,
        ScoreError::WitnessFromDifferentRequirement { expected, .. } if expected == REQUIREMENT_ID
    ));
}

#[test]
fn a_requirement_over_fewer_dimensions_can_pass_where_the_strict_one_fails() {
    let prediction = declared_frame().with(FrameDimension::SpecimenPreparation, "ffpe");
    let reference = declared_frame().with(FrameDimension::SpecimenPreparation, "fresh_frozen");

    assert!(gate(
        &ComparabilityRequirement::strict("r"),
        &prediction,
        &reference,
        &[]
    )
    .is_err());

    let narrower = ComparabilityRequirement::over(
        "bioeval/morphometric/1",
        [FrameDimension::Unit, FrameDimension::CoordinateFrame],
    );
    assert!(
        gate(&narrower, &prediction, &reference, &[]).is_ok(),
        "a task whose quantity genuinely does not depend on fixation may say so, by name"
    );
}

#[test]
fn a_comparability_failure_traces_to_the_scope_class_that_caused_it() {
    assert_eq!(
        FrameDimension::CoordinateFrame.scope_class(),
        ScopeClass::Coordinate
    );
    assert_eq!(
        FrameDimension::AssayPlatform.scope_class(),
        ScopeClass::Specimen
    );
    assert!(FrameDimension::CANONICAL
        .iter()
        .all(|d| d.scope_class().is_classified()));
}

#[test]
fn grading_across_incomparable_frames_yields_a_typed_reason_instead_of_a_number() {
    let refusal = common::grader()
        .gate_and_grade(
            &declared_frame().with(FrameDimension::Unit, "cm"),
            &declared_frame(),
            &[],
            "case-1",
            &Prediction::categorical(PROGRESSION),
            &progression_reference(Dispersion::Aleatoric),
        )
        .expect_err("centimetres against millimetres is a factor of ten, not a score");

    match refusal {
        ScoreError::Incomparable(reasons) => {
            assert_eq!(reasons[0].dimension(), FrameDimension::Unit);
        }
        other => panic!("expected an incomparability, got {other:?}"),
    }
}

#[test]
fn gating_and_grading_in_one_step_agrees_with_gating_first() {
    let combined = common::grader()
        .gate_and_grade(
            &declared_frame(),
            &declared_frame(),
            &[],
            "case-1",
            &Prediction::categorical(PROGRESSION),
            &progression_reference(Dispersion::Aleatoric),
        )
        .expect("identical frames are comparable");
    let separate = common::score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );

    assert_eq!(combined.score(), Some(&separate));
}

#[test]
fn a_direct_witness_records_no_bridge_loss_on_the_score() {
    let graded = common::score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );

    assert_eq!(graded.requirement_id(), REQUIREMENT_ID);
    assert_eq!(graded.bridge_loss(), 0.0);
    assert!(witness().is_direct());
}
