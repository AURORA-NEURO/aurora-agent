//! Adaptive test selection and stopping.
//!
//! Blueprint 35.14: "select the most informative subset from a million-instance registry under cost
//! and coverage constraints."
//!
//! ## Safety strata are taken before the budget is consulted
//!
//! 35's scale constraints say it plainly: "the scheduler may not skip mandatory safety strata for
//! information-gain efficiency." So [`AdaptivePlan::select`] takes every mandatory stratum in full
//! *first*, and only then spends what remains on discretionary strata by expected information gain.
//! A budget below the mandatory floor is [`AdaptiveError::BudgetBelowMandatoryFloor`] — the plan
//! refuses rather than dropping the cheapest safety items, because a silently truncated safety
//! stratum still produces a plausible-looking result bundle.
//!
//! ## Stopping is computed on effective size, not instance count
//!
//! [`StoppingDecision::evaluate`] takes an *effective* sample size. Feeding it an instance count
//! would shrink the confidence interval by the square root of the inflation ratio and declare
//! victory early — the statistical form of the same error the rest of this crate exists to prevent.
//! The half-width is a normal approximation, which is stated in the returned decision because it is
//! poor at extreme rates and small samples.
//!
//! ## Not implemented
//!
//! No capability posterior and no regression-risk prior. 35.14 requires both; both are models that
//! need historical run data, and a prior invented here would be a made-up number wearing a Bayesian
//! hat. Expected information gain is supplied per stratum by the caller.

use crate::error::AdaptiveError;
use serde::{Deserialize, Serialize};

/// A group of instances selected or skipped together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stratum {
    pub name: String,
    /// Mandatory strata are taken in full regardless of information gain.
    pub mandatory: bool,
    pub instances: usize,
    /// Caller-supplied expected information gain per instance. Ordering only; no units.
    pub expected_information_gain: f64,
}

impl Stratum {
    pub fn mandatory(name: impl Into<String>, instances: usize) -> Self {
        Stratum {
            name: name.into(),
            mandatory: true,
            instances,
            expected_information_gain: f64::INFINITY,
        }
    }

    pub fn discretionary(name: impl Into<String>, instances: usize, gain: f64) -> Self {
        Stratum {
            name: name.into(),
            mandatory: false,
            instances,
            expected_information_gain: gain,
        }
    }
}

/// The registry to select from, under a budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptivePlan {
    pub strata: Vec<Stratum>,
}

impl AdaptivePlan {
    pub fn new(strata: Vec<Stratum>) -> Self {
        AdaptivePlan { strata }
    }

    /// Instances that must run whatever the budget says.
    pub fn mandatory_floor(&self) -> usize {
        self.strata
            .iter()
            .filter(|stratum| stratum.mandatory)
            .map(|stratum| stratum.instances)
            .sum()
    }

    /// Selects under `budget`, mandatory strata first.
    ///
    /// Discretionary strata are taken whole in descending information gain, with ties broken by
    /// name so the selection is deterministic. A stratum that does not fit is skipped rather than
    /// partially taken: half a stratum does not have the coverage property the stratum was defined
    /// for.
    pub fn select(&self, budget: usize) -> Result<Selection, AdaptiveError> {
        let floor = self.mandatory_floor();
        if budget < floor {
            let strata = self
                .strata
                .iter()
                .filter(|stratum| stratum.mandatory)
                .map(|stratum| stratum.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(AdaptiveError::BudgetBelowMandatoryFloor {
                budget,
                floor,
                strata,
            });
        }

        let mut taken: Vec<String> = self
            .strata
            .iter()
            .filter(|stratum| stratum.mandatory)
            .map(|stratum| stratum.name.clone())
            .collect();
        let mut instances = floor;

        let mut discretionary: Vec<&Stratum> = self
            .strata
            .iter()
            .filter(|stratum| !stratum.mandatory)
            .collect();
        discretionary.sort_by(|a, b| {
            b.expected_information_gain
                .partial_cmp(&a.expected_information_gain)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
        });

        let mut skipped = Vec::new();
        for stratum in discretionary {
            if instances + stratum.instances <= budget {
                instances += stratum.instances;
                taken.push(stratum.name.clone());
            } else {
                skipped.push(stratum.name.clone());
            }
        }

        Ok(Selection {
            selected_strata: taken,
            skipped_strata: skipped,
            instances,
            mandatory_instances: floor,
            budget,
        })
    }
}

/// What will run, and what will not. 35's constraint that "results name the subset actually
/// executed" is why the skipped strata are carried rather than dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub selected_strata: Vec<String>,
    pub skipped_strata: Vec<String>,
    pub instances: usize,
    pub mandatory_instances: usize,
    pub budget: usize,
}

impl Selection {
    /// Instances saved against running the whole registry.
    pub fn cost_reduction_versus_exhaustive(&self, registry_size: usize) -> f64 {
        if registry_size == 0 {
            0.0
        } else {
            1.0 - (self.instances as f64 / registry_size as f64)
        }
    }
}

/// Whether enough has been run, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoppingDecision {
    pub stop: bool,
    pub rate: f64,
    pub half_width: f64,
    pub target_half_width: f64,
    /// The effective sample size the interval was computed on, carried so a reader can check it was
    /// not the instance count.
    pub effective_sample_size: f64,
    pub reason: String,
}

/// z for a two-sided 95% interval. Fixed rather than configurable: one confidence level, stated.
const Z_95: f64 = 1.96;

impl StoppingDecision {
    /// Normal-approximation sequential stop on an effective sample size.
    pub fn evaluate(
        successes: usize,
        effective_sample_size: f64,
        target_half_width: f64,
    ) -> Result<Self, AdaptiveError> {
        if target_half_width <= 0.0 {
            return Err(AdaptiveError::NonPositiveTarget(target_half_width));
        }
        if effective_sample_size <= 0.0 {
            return Ok(StoppingDecision {
                stop: false,
                rate: 0.0,
                half_width: f64::INFINITY,
                target_half_width,
                effective_sample_size,
                reason: "no effective observations yet".into(),
            });
        }

        let rate = (successes as f64 / effective_sample_size).clamp(0.0, 1.0);
        let half_width = Z_95 * (rate * (1.0 - rate) / effective_sample_size).sqrt();
        let stop = half_width <= target_half_width;
        Ok(StoppingDecision {
            stop,
            rate,
            half_width,
            target_half_width,
            effective_sample_size,
            reason: if stop {
                format!(
                    "95% half-width {half_width:.4} is within the {target_half_width:.4} target on \
                     {effective_sample_size:.1} effective observations (normal approximation)"
                )
            } else {
                format!(
                    "95% half-width {half_width:.4} exceeds the {target_half_width:.4} target on \
                     {effective_sample_size:.1} effective observations (normal approximation)"
                )
            },
        })
    }
}
