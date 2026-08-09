//! What a decision procedure concludes, and what kind of claim that is.
//!
//! A stress program needs something to stress. This module supplies a small set of reference
//! procedures — not models, but the summaries a research report actually states — so that the
//! metamorphic relations in [`crate::relation`] have concrete quantities to constrain.
//!
//! The taxonomy in [`Character`] is the load-bearing idea, and it is the one 32.07's worked
//! relation turns on: *"ranking may remain stable while calibration and decision thresholds must
//! change"*. A discriminative summary answers "who is higher risk than whom"; a calibrated one
//! answers "how likely is this, here". Under a prevalence shift the first is required to stay
//! exactly put and the second is required to move by a computable amount. Reporting a single
//! "robustness score" over both would average a correct invariance against a correct change and
//! call the result fragility.
//!
//! These procedures are deliberately closed-form and deterministic. Nothing here is fitted, so
//! nothing here can absorb a perturbation by refitting — which is what makes the observed changes
//! attributable to the stress rather than to an optimiser.

use crate::cohort::{Cohort, Ranked};
use crate::error::StressError;
use serde::{Deserialize, Serialize};

/// The kind of claim a procedure makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Character {
    /// Orders or separates subjects. Says nothing about absolute risk.
    Discriminative,
    /// Assigns absolute risk or a decision quantity. Depends on the base rate by construction.
    Calibrated,
    /// Reads a segmentation-derived geometric quantity.
    Geometric,
}

/// A closed-form summary of a cohort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "procedure", rename_all = "snake_case")]
pub enum Procedure {
    /// Subjects ordered by marker. The purest discriminative claim.
    MarkerRanking,
    /// Weighted rank concordance of marker against condition.
    MarkerSeparation,
    /// Weighted difference of class means. An effect size, not a decision.
    GroupContrast,
    /// Mean posterior log-odds under a fixed likelihood slope and the cohort's own base rate.
    ///
    /// The prior enters additively in log-odds, which is what makes its response to a prevalence
    /// shift exactly predictable rather than merely directional.
    CalibratedLogOdds { slope: f64, reference: f64 },
    /// Weighted positive predictive value of a fixed marker threshold.
    ///
    /// The quantity a clinic actually acts on, and the one that has no meaning without a base
    /// rate. 32.07: *"accuracy changes interpreted as capability change"* is what happens when
    /// this is reported as though it were discriminative.
    PositivePredictiveValue { threshold: f64 },
    /// Subjects whose segmentation-derived volume clears a threshold.
    VolumeThreshold { mm3: f64 },
    /// Subjects ordered by segmentation-derived volume.
    ///
    /// Kept separate from [`Procedure::VolumeThreshold`] because the two break differently: an
    /// ordering survives jitter everywhere except between near-equal volumes, while a threshold
    /// survives it everywhere except at the threshold. Collapsing them would hide which of the two
    /// a given report actually relies on.
    VolumeRanking,
}

impl Procedure {
    pub fn character(&self) -> Character {
        match self {
            Procedure::MarkerRanking
            | Procedure::MarkerSeparation
            | Procedure::GroupContrast => Character::Discriminative,
            Procedure::CalibratedLogOdds { .. } | Procedure::PositivePredictiveValue { .. } => {
                Character::Calibrated
            }
            Procedure::VolumeThreshold { .. } | Procedure::VolumeRanking => Character::Geometric,
        }
    }

    /// A stable identifier, used as the key of a robustness finding.
    pub fn id(&self) -> String {
        match self {
            Procedure::MarkerRanking => "marker_ranking".into(),
            Procedure::MarkerSeparation => "marker_separation".into(),
            Procedure::GroupContrast => "group_contrast".into(),
            Procedure::CalibratedLogOdds { slope, reference } => {
                format!("calibrated_log_odds(slope={slope:.4},reference={reference:.4})")
            }
            Procedure::PositivePredictiveValue { threshold } => {
                format!("positive_predictive_value(threshold={threshold:.4})")
            }
            Procedure::VolumeThreshold { mm3 } => format!("volume_threshold(mm3={mm3:.4})"),
            Procedure::VolumeRanking => "volume_ranking".into(),
        }
    }

    /// Evaluates the procedure.
    pub fn conclude(&self, cohort: &Cohort) -> Result<Conclusion, StressError> {
        let undefined = |reason: &str| StressError::ConclusionUndefined {
            procedure: self.id(),
            cohort: cohort.id.clone(),
            reason: reason.to_string(),
        };
        let value = match self {
            Procedure::MarkerRanking => ConclusionValue::Ordering(cohort.ranking()),
            Procedure::VolumeRanking => ConclusionValue::Ordering(cohort.volume_ranking()),
            Procedure::MarkerSeparation => ConclusionValue::Scalar(
                cohort
                    .separation()
                    .ok_or_else(|| undefined("one class has no analysable subjects"))?,
            ),
            Procedure::GroupContrast => {
                let positive = cohort
                    .class_mean(true)
                    .ok_or_else(|| undefined("no analysable positive subjects"))?;
                let negative = cohort
                    .class_mean(false)
                    .ok_or_else(|| undefined("no analysable negative subjects"))?;
                ConclusionValue::Scalar(positive - negative)
            }
            Procedure::CalibratedLogOdds { slope, reference } => {
                let analysable = cohort.resolved().count();
                if analysable == 0 {
                    return Err(undefined("every subject is below the limit of detection"));
                }
                let prior = logit(cohort.prevalence())
                    .ok_or_else(|| undefined("base rate is degenerate, so log-odds are infinite"))?;
                let total: f64 = cohort
                    .resolved()
                    .map(|subject| prior + slope * (subject.marker - reference))
                    .sum();
                ConclusionValue::Scalar(total / analysable as f64)
            }
            Procedure::PositivePredictiveValue { threshold } => {
                let flagged: f64 = cohort
                    .resolved()
                    .filter(|subject| subject.marker >= *threshold)
                    .map(|subject| subject.weight)
                    .sum();
                if flagged <= 0.0 {
                    return Err(undefined("no analysable subject clears the threshold"));
                }
                let true_positive: f64 = cohort
                    .resolved()
                    .filter(|subject| subject.marker >= *threshold && subject.condition)
                    .map(|subject| subject.weight)
                    .sum();
                ConclusionValue::Scalar(true_positive / flagged)
            }
            Procedure::VolumeThreshold { mm3 } => {
                let mut flagged: Vec<String> = cohort
                    .resolved()
                    .filter(|subject| subject.volume_mm3 >= *mm3)
                    .map(|subject| subject.id.clone())
                    .collect();
                flagged.sort();
                ConclusionValue::Membership(flagged)
            }
        };
        Ok(Conclusion {
            id: self.id(),
            character: self.character(),
            value,
            unresolved: cohort.unresolved_count(),
        })
    }
}

/// The value of a conclusion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "value", rename_all = "snake_case")]
pub enum ConclusionValue {
    Scalar(f64),
    Ordering(Vec<Ranked>),
    Membership(Vec<String>),
}

impl ConclusionValue {
    pub fn as_scalar(&self) -> Option<f64> {
        match self {
            ConclusionValue::Scalar(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_ordering(&self) -> Option<&[Ranked]> {
        match self {
            ConclusionValue::Ordering(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_membership(&self) -> Option<&[String]> {
        match self {
            ConclusionValue::Membership(items) => Some(items),
            _ => None,
        }
    }

    pub fn ids(&self) -> Vec<&str> {
        match self {
            ConclusionValue::Scalar(_) => Vec::new(),
            ConclusionValue::Ordering(items) => {
                items.iter().map(|item| item.id.as_str()).collect()
            }
            ConclusionValue::Membership(items) => items.iter().map(String::as_str).collect(),
        }
    }

    /// A short description, with long sequences elided.
    ///
    /// A violation message that prints forty subject identifiers buries the one that moved. The
    /// relations that can localise a difference do so themselves; this is the fallback.
    pub fn describe(&self) -> String {
        const SHOWN: usize = 5;
        let elide = |items: Vec<&str>| {
            if items.len() <= SHOWN {
                items.join(", ")
            } else {
                format!(
                    "{}, … ({} in total)",
                    items[..SHOWN].join(", "),
                    items.len()
                )
            }
        };
        match self {
            ConclusionValue::Scalar(value) => format!("{value:.6}"),
            ConclusionValue::Ordering(_) => format!("order [{}]", elide(self.ids())),
            ConclusionValue::Membership(_) => format!("set {{{}}}", elide(self.ids())),
        }
    }
}

/// One stated finding.
///
/// `unresolved` travels with the conclusion because a summary computed after the assay lost a
/// tenth of the cohort is a different claim from the same number computed on everyone, and 32.03's
/// characteristic failure is exactly the habit of not saying so.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conclusion {
    pub id: String,
    pub character: Character,
    pub value: ConclusionValue,
    pub unresolved: usize,
}

/// Log-odds, or `None` at a degenerate base rate where they do not exist.
pub fn logit(probability: f64) -> Option<f64> {
    if probability > 0.0 && probability < 1.0 {
        Some((probability / (1.0 - probability)).ln())
    } else {
        None
    }
}
