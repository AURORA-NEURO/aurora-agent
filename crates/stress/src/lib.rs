//! Biological stress: perturbing the data-generating process, not the split.
//!
//! Implements the distribution-shift and measurement-degradation half of blueprint section 32,
//! the Biological Mutation and Stress Program, and closes a gap the platform's own audit found.
//! Blueprint 38.01 names six mutation families for the radiogenomic reference world; the platform
//! had generator knobs for four of them, all leakage mechanisms. Prevalence shift, segmentation
//! perturbation and assay uncertainty had none. [`coverage`] is the machine-checked record that
//! all six now do.
//!
//! **The boundary with `bioprism-mutation`.** That crate injects and repairs *split defects* —
//! identity, site, temporal and preprocessing leakage — each with an oracle witness that appears or
//! disappears. Its question is "is this benchmark instance sound?". This crate leaves the split
//! alone and changes the world the data came from: the base rate, the laboratory, the assay's
//! precision, the contour a volume was measured from. Its question is "does this conclusion depend
//! on a fact about the world that could plausibly have been otherwise?". The two never overlap and
//! neither reimplements the other.
//!
//! **The verdict is a breaking point, not a score.** Running a family produces a
//! [`RobustnessProfile`]: for each conclusion, the largest intensity at which its declared relation
//! still held and the intensity at which it failed. "Seventeen of twenty conclusions survived" is
//! not a finding. "The subtype contrast survived a batch offset of one and a quarter standard
//! deviations and failed at one and a half" is.
//!
//! **Every stress declares an executable postcondition and is checked against it.** Two layers:
//! [`CohortInvariant`] constrains what the generator did to the cohort, and [`StressRelation`]
//! constrains what a conclusion may do in response. Both are run, and a failure of the first
//! abandons the rung rather than scoring it — a number produced by a broken generator is worse
//! than a missing one.
//!
//! **What is deliberately not here.** No model is fitted; the procedures in [`conclusion`] are
//! closed-form summaries, so an observed change is attributable to the stress rather than to an
//! optimiser. No preanalytic, ontology-drift or causal-collider families (32.04, 32.09, 32.11):
//! each needs state this crate does not model, and a family that cannot state an executable
//! postcondition is not a test. No temporal stress: `bioprism-mutation` already owns it, and two
//! weaker answers to one question are worse than one.

pub mod cohort;
pub mod conclusion;
pub mod coverage;
pub mod error;
pub mod family;
pub mod invariant;
pub mod perturb;
pub mod profile;
pub mod relation;
pub mod rng;

pub use cohort::{Cohort, Ranked, Subject};
pub use conclusion::{Character, Conclusion, ConclusionValue, Procedure};
pub use coverage::{coverage, Coverage, FamilyOwner, ReferenceFamily};
pub use error::StressError;
pub use family::{Knob, Magnitude, Stress, StressFamily};
pub use invariant::{CohortInvariant, InvariantCheck, PostconditionResult};
pub use perturb::{apply, perturb, postconditions, validate, Perturbed};
pub use profile::{
    profile, ConclusionRobustness, GeneratorDefect, Identifiability, RobustnessProfile,
    StressReport, SweepPoint,
};
pub use relation::{declare, DeclaredRelation, Direction, Obligation, StressRelation};

/// The reference reading panel: one procedure per kind of claim a report makes.
///
/// Seven summaries covering all three characters, chosen so that no two respond identically to any
/// family. A panel of near-duplicates would report a broad-looking robustness profile built from
/// one measurement, which is the effective-diversity mistake `bioprism-mutation` documents for
/// instance counts, transplanted to conclusions.
pub fn standard_panel() -> Vec<Procedure> {
    vec![
        Procedure::MarkerRanking,
        Procedure::MarkerSeparation,
        Procedure::GroupContrast,
        Procedure::CalibratedLogOdds {
            slope: 1.0,
            reference: 0.0,
        },
        Procedure::PositivePredictiveValue { threshold: 0.5 },
        Procedure::VolumeThreshold { mm3: 1_000.0 },
        Procedure::VolumeRanking,
    ]
}

/// One stress from each family, at the settings a research protocol would state.
///
/// The numbers are deliberately ordinary rather than adversarial: a deployment base rate an order
/// of magnitude below an enriched case-control cohort, a site offset of one pooled within-class
/// standard deviation, an assay three times noisier than the one the claim was made on, and
/// segmentation jitter equal to a typical published test-retest coefficient of variation. A stress
/// program that only breaks conclusions at implausible magnitudes has not tested anything.
pub fn standard_program(batch: impl Into<String>) -> Vec<Stress> {
    let batch = batch.into();
    vec![
        Stress::new(
            "prevalence-to-deployment",
            Knob::PrevalenceShift {
                target_prevalence: 0.05,
            },
            Magnitude::FULL,
            20_260_808,
        ),
        Stress::new(
            "site-offset",
            Knob::BatchEffect {
                batch,
                offset_sd: 1.0,
            },
            Magnitude::FULL,
            20_260_809,
        ),
        Stress::new(
            "assay-precision-loss",
            Knob::AssayDegradation {
                sd_multiplier: 3.0,
                limit_of_detection: None,
            },
            Magnitude::FULL,
            20_260_810,
        ),
        Stress::new(
            "segmentation-reproducibility",
            Knob::SegmentationJitter {
                reproducibility_cv: 0.05,
            },
            Magnitude::FULL,
            20_260_811,
        ),
    ]
}
