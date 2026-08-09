//! Difficulty calibration: what the observed pass rates establish, and what they do not.
//!
//! 15.00 makes difficulty calibration part of pack eligibility and asks the coverage matrix to
//! track a difficulty distribution; 15.19 lists "statistical diversity and calibration analysis"
//! among its oracles. Neither says what the numbers must clear, so the thresholds here are stated
//! as a policy struct with documented defaults rather than buried as constants.
//!
//! The question this module answers is narrow: **does this pack separate anything?** A pack every
//! system passes and a pack every system fails are equally uninformative, and both are common
//! outcomes — the first from age, the second from a broken environment. Reporting either as a
//! score is worse than reporting nothing, because a number invites comparison.
//!
//! Separation is screened with Wilson score intervals on the best and worst systems. That is a
//! deliberately conservative check and it is *not* a hypothesis test: it takes no account of
//! multiplicity across packs, of correlation between trials of the same system, or of the fact
//! that the systems compared were usually chosen after seeing some of the results. It can say
//! "these intervals overlap, so the ordering is not established". It cannot say the reverse with
//! any strength beyond that of the sample.

use crate::error::PackError;
use serde::{Deserialize, Serialize};

/// z for a two-sided 95% normal interval.
const Z_95: f64 = 1.959_963_984_540_054;

/// One system's record against one pack.
///
/// Counts, not rates: a rate discards the denominator, and the denominator is what decides
/// whether the rate means anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemObservation {
    pub system: String,
    pub trials: u32,
    pub passes: u32,
}

impl SystemObservation {
    pub fn new(system: impl Into<String>, passes: u32, trials: u32) -> Result<Self, PackError> {
        let system = system.into();
        if passes > trials {
            return Err(PackError::ImpossibleObservation {
                system,
                passes,
                trials,
            });
        }
        Ok(SystemObservation {
            system,
            trials,
            passes,
        })
    }

    /// `None` when no trials were run. An absent measurement is not a zero.
    pub fn pass_rate(&self) -> Option<f64> {
        if self.trials == 0 {
            None
        } else {
            Some(self.passes as f64 / self.trials as f64)
        }
    }

    /// Wilson score interval at 95%.
    ///
    /// Chosen over the normal approximation because pass rates near 0 and near 1 are exactly the
    /// cases this crate cares about, and that is where the normal interval misbehaves worst
    /// (it produces bounds outside [0, 1] and collapses to zero width at p = 0 or p = 1).
    pub fn wilson_interval(&self) -> Option<(f64, f64)> {
        let p = self.pass_rate()?;
        let n = self.trials as f64;
        let z2 = Z_95 * Z_95;
        let denominator = 1.0 + z2 / n;
        let centre = (p + z2 / (2.0 * n)) / denominator;
        let spread = (Z_95 / denominator) * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
        Some(((centre - spread).max(0.0), (centre + spread).min(1.0)))
    }
}

/// Thresholds for calling a pack saturated, floored or too thin to judge.
///
/// These are conventions, not derived quantities. 0.95 and 0.05 are the points past which the
/// remaining headroom is smaller than the run-to-run noise of most agent evaluations; three
/// systems is the smallest number for which "everyone passes" is a statement about the pack
/// rather than about one system.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CalibrationPolicy {
    pub saturation_ceiling: f64,
    pub floor: f64,
    pub min_systems: usize,
    pub min_trials_per_system: u32,
}

impl Default for CalibrationPolicy {
    fn default() -> Self {
        CalibrationPolicy {
            saturation_ceiling: 0.95,
            floor: 0.05,
            min_systems: 3,
            min_trials_per_system: 20,
        }
    }
}

/// What the observed pass rates establish about the pack's discriminating power.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Discrimination {
    /// Not enough evidence about the pack yet. Carries the reason so a release note cannot
    /// silently render this as "no problems found".
    Undetermined { reason: String },
    /// Everyone passes. The pack orders nothing above the ceiling.
    Saturated {
        pooled_pass_rate: f64,
        systems: usize,
    },
    /// Everyone fails. Usually a broken environment rather than a hard benchmark.
    Floored {
        pooled_pass_rate: f64,
        systems: usize,
    },
    /// The observed pass rates span a range. `separated` is true only when the best and worst
    /// systems' 95% Wilson intervals are disjoint.
    Discriminating {
        lowest: f64,
        highest: f64,
        separated: bool,
    },
}

impl Discrimination {
    pub fn is_discriminating(&self) -> bool {
        matches!(self, Discrimination::Discriminating { .. })
    }
}

/// The pass-rate record for one pack.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DifficultyCalibration {
    pub observations: Vec<SystemObservation>,
}

impl DifficultyCalibration {
    pub fn new(observations: Vec<SystemObservation>) -> Self {
        DifficultyCalibration { observations }
    }

    pub fn total_trials(&self) -> u32 {
        self.observations.iter().map(|o| o.trials).sum()
    }

    /// Passes over trials across every system. `None` when no trials were run.
    ///
    /// Pooling weights systems by how often they were run, which is rarely what a reader assumes.
    /// It is used here only for the saturation and floor checks, where the question is whether
    /// *any* headroom remains rather than how systems rank.
    pub fn pooled_pass_rate(&self) -> Option<f64> {
        let trials = self.total_trials();
        if trials == 0 {
            return None;
        }
        let passes: u32 = self.observations.iter().map(|o| o.passes).sum();
        Some(passes as f64 / trials as f64)
    }

    pub fn best(&self) -> Option<&SystemObservation> {
        self.observations
            .iter()
            .filter(|o| o.trials > 0)
            .max_by(|a, b| rate_order(a, b))
    }

    pub fn worst(&self) -> Option<&SystemObservation> {
        self.observations
            .iter()
            .filter(|o| o.trials > 0)
            .min_by(|a, b| rate_order(a, b))
    }

    /// The observed pass-rate range, or an explicit statement that there is not one.
    ///
    /// A pack where everything passes or everything fails discriminates nothing, and that is
    /// reported as its own verdict rather than as a range of width zero.
    pub fn discrimination(&self, policy: &CalibrationPolicy) -> Discrimination {
        let measured: Vec<&SystemObservation> = self
            .observations
            .iter()
            .filter(|o| o.trials >= policy.min_trials_per_system)
            .collect();

        if measured.len() < policy.min_systems {
            return Discrimination::Undetermined {
                reason: format!(
                    "{} system(s) with at least {} trials; {} required before the pack itself can \
                     be characterised",
                    measured.len(),
                    policy.min_trials_per_system,
                    policy.min_systems
                ),
            };
        }

        let Some(pooled) = self.pooled_pass_rate() else {
            return Discrimination::Undetermined {
                reason: "no trials recorded".to_string(),
            };
        };

        if pooled >= policy.saturation_ceiling {
            return Discrimination::Saturated {
                pooled_pass_rate: pooled,
                systems: measured.len(),
            };
        }
        if pooled <= policy.floor {
            return Discrimination::Floored {
                pooled_pass_rate: pooled,
                systems: measured.len(),
            };
        }

        let (Some(best), Some(worst)) = (self.best(), self.worst()) else {
            return Discrimination::Undetermined {
                reason: "no system has any trials".to_string(),
            };
        };
        let separated = match (worst.wilson_interval(), best.wilson_interval()) {
            (Some((_, worst_high)), Some((best_low, _))) => worst_high < best_low,
            _ => false,
        };

        Discrimination::Discriminating {
            lowest: worst.pass_rate().unwrap_or(0.0),
            highest: best.pass_rate().unwrap_or(0.0),
            separated,
        }
    }

    /// One sentence stating the range and its caveat.
    pub fn report(&self, policy: &CalibrationPolicy) -> String {
        match self.discrimination(policy) {
            Discrimination::Undetermined { reason } => {
                format!("discriminating range undetermined: {reason}")
            }
            Discrimination::Saturated {
                pooled_pass_rate,
                systems,
            } => format!(
                "saturated at {pooled_pass_rate:.3} pooled across {systems} system(s); the pack \
                 orders nothing and must not be reported as a score"
            ),
            Discrimination::Floored {
                pooled_pass_rate,
                systems,
            } => format!(
                "floored at {pooled_pass_rate:.3} pooled across {systems} system(s); the pack \
                 orders nothing, and a floor is more often a broken environment than a hard task"
            ),
            Discrimination::Discriminating {
                lowest,
                highest,
                separated,
            } => {
                let caveat = if separated {
                    "best and worst 95% Wilson intervals are disjoint; this is a conservative \
                     screen, not a multiplicity-corrected test"
                } else {
                    "best and worst 95% Wilson intervals overlap, so the ordering is not \
                     established by these trials"
                };
                format!("observed pass rates span {lowest:.3}–{highest:.3}; {caveat}")
            }
        }
    }
}

fn rate_order(a: &SystemObservation, b: &SystemObservation) -> std::cmp::Ordering {
    let left = a.pass_rate().unwrap_or(0.0);
    let right = b.pass_rate().unwrap_or(0.0);
    left.partial_cmp(&right)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.system.cmp(&b.system))
}
