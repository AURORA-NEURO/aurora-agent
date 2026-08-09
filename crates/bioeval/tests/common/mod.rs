#![allow(dead_code)]

//! Fixtures shared by the invariant suites.
//!
//! The recurring fixture is 31.01's worked case: a progression assessment whose reference
//! standard is `0.55 progression / 0.35 treatment-effect / 0.10 mixed`. It appears in most of
//! these tests because it is the case the blueprint itself uses to argue that a calibrated
//! forecast can beat a categorical label, and reproducing that ordering is the crate's headline
//! claim.

use bioprism_bioeval::{
    gate, ComparabilityRequirement, ComparabilityWitness, Dispersion, FrameDimension, Grader,
    MeasurementFrame, PredictedDistribution, Prediction, ReferenceDistribution, ReferenceStandard,
};

pub const PROGRESSION: &str = "progression";
pub const TREATMENT_EFFECT: &str = "treatment_effect";
pub const MIXED: &str = "mixed";

pub const REQUIREMENT_ID: &str = "bioeval/strict/1";

/// A fully declared measurement frame. Both sides of the fixture comparisons use it, so that
/// tests about scoring are not accidentally tests about the comparability gate.
pub fn declared_frame() -> MeasurementFrame {
    MeasurementFrame::default()
        .with(FrameDimension::AssayPlatform, "mri-3t-flair")
        .with(FrameDimension::ReferenceBuild, "n/a")
        .with(FrameDimension::CoordinateFrame, "ras")
        .with(FrameDimension::Unit, "mm")
        .with(FrameDimension::Normalisation, "none")
        .with(FrameDimension::SpecimenPreparation, "in_vivo")
}

pub fn requirement() -> ComparabilityRequirement {
    ComparabilityRequirement::strict(REQUIREMENT_ID)
}

/// A witness earned honestly: two identical, fully declared frames under the strict requirement.
pub fn witness() -> ComparabilityWitness {
    gate(&requirement(), &declared_frame(), &declared_frame(), &[])
        .expect("identical fully declared frames are comparable")
}

pub fn grader() -> Grader {
    Grader::new("bioeval/fixture-grader", requirement())
}

/// 31.01's worked case, with the spread declared to be real biology.
pub fn progression_reference(dispersion: Dispersion) -> ReferenceStandard {
    ReferenceStandard::Distribution(
        ReferenceDistribution::new(
            [
                (PROGRESSION.to_string(), 0.55),
                (TREATMENT_EFFECT.to_string(), 0.35),
                (MIXED.to_string(), 0.10),
            ],
            dispersion,
        )
        .expect("the fixture masses sum to one"),
    )
}

/// A reference standard that genuinely admits one answer.
pub fn resolved_reference(state: &str) -> ReferenceStandard {
    ReferenceStandard::Distribution(ReferenceDistribution::resolved(state))
}

/// A reference at exactly the confidence the task description calls out: 60% on its own modal
/// answer.
pub fn sixty_percent_reference() -> ReferenceStandard {
    ReferenceStandard::Distribution(
        ReferenceDistribution::new(
            [
                (PROGRESSION.to_string(), 0.6),
                (TREATMENT_EFFECT.to_string(), 0.4),
            ],
            Dispersion::Aleatoric,
        )
        .expect("the fixture masses sum to one"),
    )
}

/// The calibrated forecast from 31.01's worked case.
pub fn calibrated_forecast() -> Prediction {
    Prediction::Distributional(
        PredictedDistribution::new([
            (PROGRESSION.to_string(), 0.55),
            (TREATMENT_EFFECT.to_string(), 0.35),
            (MIXED.to_string(), 0.10),
        ])
        .expect("the fixture masses sum to one"),
    )
}

pub fn forecast(entries: [(&str, f64); 3]) -> Prediction {
    Prediction::Distributional(
        PredictedDistribution::new(entries.map(|(s, m)| (s.to_string(), m)))
            .expect("fixture forecasts are normalised"),
    )
}

/// Grades a prediction against a reference, panicking on refusal. Tests that care about the
/// refusal call `Grader::grade` directly.
pub fn score(prediction: &Prediction, reference: &ReferenceStandard) -> bioprism_bioeval::BioScore {
    grader()
        .grade(&witness(), "case-1", prediction, reference)
        .expect("the fixture grades cleanly")
        .score()
        .expect("the fixture prediction is not an abstention")
        .clone()
}
