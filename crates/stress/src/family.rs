//! The four stress families and their intensity knob.
//!
//! Blueprint 32 types a transformation along five axes — representation, observation, latent
//! biology, workflow, required conclusion. Every family here sits on the **observation** axis and
//! on nothing else: the latent truth is never edited. That is the whole boundary between this
//! crate and a benchmark generator. `bioprism-mutation` owns the transformations that inject or
//! repair a *split defect*; these change the data-generating process while leaving the correct
//! answer about the biology exactly where it was.
//!
//! Families implemented, with the blueprint module each answers to:
//!
//! - [`StressFamily::PrevalenceShift`] — 32.07, cohort composition and sampling.
//! - [`StressFamily::BatchEffect`] — 32.06, site, batch, platform and laboratory shift.
//! - [`StressFamily::AssayDegradation`] — 32.03, assay noise, depth and detection limit.
//! - [`StressFamily::SegmentationJitter`] — 30.05 geometric evidence, entering 32 through 32.02's
//!   representation-preserving transformations.
//!
//! Deliberately not implemented here, with reasons: preanalytic storage stress (32.04) needs a
//! specimen lifecycle this crate does not model; ontology drift (32.09) is a vocabulary problem
//! with no numeric magnitude to sweep; confounding and collider stress (32.11) requires a causal
//! graph and belongs with `bioprism-graph`, not with a cohort table. Each would be a plausible
//! addition and none of them can be given an executable postcondition with what is modelled here,
//! which is the bar 32.22 sets.

use serde::{Deserialize, Serialize};

/// Stress intensity, in permille.
///
/// An integer on purpose. Breaking points are collected into ordered, deduplicated collections and
/// compared for equality; a floating-point intensity makes both operations depend on how the
/// magnitude happened to be computed. Permille is finer than any ladder this crate sweeps, so
/// nothing is lost by giving up the fractional part.
/// The field is private and the only constructors clamp or reject, so a magnitude outside
/// `[0, 1]` is unrepresentable rather than merely undocumented. Full magnitude means "the whole of
/// the stress its author declared"; a magnitude above it would silently exceed the knob the
/// postconditions were written against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct Magnitude {
    permille: u32,
}

impl Magnitude {
    pub const ZERO: Magnitude = Magnitude { permille: 0 };
    pub const FULL: Magnitude = Magnitude { permille: 1_000 };

    /// Saturating. A sweep that computes a rung slightly past the top should stop at the top, not
    /// fail; input arriving from outside goes through [`TryFrom`], which refuses instead.
    pub fn from_permille(permille: u32) -> Self {
        Magnitude {
            permille: permille.min(1_000),
        }
    }

    pub fn permille(self) -> u32 {
        self.permille
    }

    pub fn fraction(self) -> f64 {
        self.permille as f64 / 1_000.0
    }

    /// The intensities a robustness sweep visits.
    ///
    /// Eight rungs from an eighth of the declared knob to the whole of it. The rung spacing is the
    /// resolution of every breaking point this crate reports, and [`crate::profile`] says so in
    /// its caveat rather than implying a precision the sweep does not have.
    pub fn ladder() -> Vec<Magnitude> {
        (1..=8)
            .map(|step| Magnitude::from_permille(step * 125))
            .collect()
    }
}

impl std::fmt::Display for Magnitude {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.3}", self.fraction())
    }
}

impl TryFrom<u32> for Magnitude {
    type Error = crate::error::StressError;

    fn try_from(permille: u32) -> Result<Self, Self::Error> {
        if permille <= 1_000 {
            Ok(Magnitude { permille })
        } else {
            Err(crate::error::StressError::MagnitudeOutOfRange { permille })
        }
    }
}

impl From<Magnitude> for u32 {
    fn from(magnitude: Magnitude) -> u32 {
        magnitude.permille
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StressFamily {
    PrevalenceShift,
    BatchEffect,
    AssayDegradation,
    SegmentationJitter,
}

impl StressFamily {
    pub const ALL: [StressFamily; 4] = [
        StressFamily::PrevalenceShift,
        StressFamily::BatchEffect,
        StressFamily::AssayDegradation,
        StressFamily::SegmentationJitter,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            StressFamily::PrevalenceShift => "prevalence_shift",
            StressFamily::BatchEffect => "batch_effect",
            StressFamily::AssayDegradation => "assay_degradation",
            StressFamily::SegmentationJitter => "segmentation_jitter",
        }
    }

    /// The blueprint module that specifies this family.
    pub fn blueprint_module(self) -> &'static str {
        match self {
            StressFamily::PrevalenceShift => "32.07",
            StressFamily::BatchEffect => "32.06",
            StressFamily::AssayDegradation => "32.03",
            StressFamily::SegmentationJitter => "30.05",
        }
    }

    /// The sentence that says what surviving this family buys you.
    pub fn claim(self) -> &'static str {
        match self {
            StressFamily::PrevalenceShift => {
                "A conclusion that is invariant under prevalence shift is about discrimination; a \
                 conclusion that moves with prevalence is about decision. Both are legitimate and \
                 they are not interchangeable."
            }
            StressFamily::BatchEffect => {
                "A conclusion that survives a batch offset is about the biology; a conclusion that \
                 flips was reading the batch."
            }
            StressFamily::AssayDegradation => {
                "A conclusion that survives degraded precision is stronger; a conclusion that \
                 vanishes was resting on precision the assay does not have."
            }
            StressFamily::SegmentationJitter => {
                "A conclusion that survives jitter within the segmentation's own reproducibility is \
                 a claim about anatomy; a conclusion that flips inside that band is a claim about \
                 one particular contour."
            }
        }
    }
}

/// The family-specific parameters, stated at full magnitude.
///
/// A [`Stress`] carries the knob at its full setting plus a [`Magnitude`]; the perturbation
/// interpolates between identity and the full setting. Keeping the endpoint fixed and sweeping the
/// interpolation is what makes breaking points across families comparable — every family's
/// magnitude 1.0 means "the whole of the stress its author declared", not "some absolute number
/// that happens to be similar".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "knob", rename_all = "snake_case")]
pub enum Knob {
    /// Reweight to this base rate. Deployment prevalence is typically far below the enriched base
    /// rate of a case-control cohort, which is exactly why the family exists.
    PrevalenceShift { target_prevalence: f64 },
    /// Add this many pooled within-class standard deviations to every marker in one batch.
    BatchEffect { batch: String, offset_sd: f64 },
    /// Multiply the within-class spread by this factor, censoring anything that falls below the
    /// limit of detection.
    AssayDegradation {
        sd_multiplier: f64,
        limit_of_detection: Option<f64>,
    },
    /// Jitter volumes within this coefficient of variation, the segmentation's stated test-retest
    /// reproducibility.
    SegmentationJitter { reproducibility_cv: f64 },
}

impl Knob {
    pub fn family(&self) -> StressFamily {
        match self {
            Knob::PrevalenceShift { .. } => StressFamily::PrevalenceShift,
            Knob::BatchEffect { .. } => StressFamily::BatchEffect,
            Knob::AssayDegradation { .. } => StressFamily::AssayDegradation,
            Knob::SegmentationJitter { .. } => StressFamily::SegmentationJitter,
        }
    }
}

/// One parameterised, seeded, reproducible stress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stress {
    pub id: String,
    pub knob: Knob,
    pub magnitude: Magnitude,
    pub seed: u64,
}

impl Stress {
    pub fn new(id: impl Into<String>, knob: Knob, magnitude: Magnitude, seed: u64) -> Self {
        Stress {
            id: id.into(),
            knob,
            magnitude,
            seed,
        }
    }

    pub fn family(&self) -> StressFamily {
        self.knob.family()
    }

    /// The same stress at a different intensity, seed and knob unchanged.
    pub fn at(&self, magnitude: Magnitude) -> Stress {
        Stress {
            id: format!("{}@{}", self.id, magnitude),
            knob: self.knob.clone(),
            magnitude,
            seed: self.seed,
        }
    }
}
