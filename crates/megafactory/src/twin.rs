//! Mechanistic simulation and the assay-twin factory.
//!
//! Blueprint 35.04: "generate worlds with known dynamics and counterfactuals **while quantifying
//! model discrepancy**." The second clause is the whole module. A simulator that reports a
//! counterfactual without its discrepancy has manufactured ground truth, and a benchmark built on
//! manufactured ground truth grades systems on their agreement with one modeller's assumptions.
//!
//! ## There is no variant that means "true"
//!
//! [`TwinTruth`] has one variant, [`TwinTruth::UnderModel`], and it carries the model id and that
//! model's stated misspecification. No constructor produces a model-free answer, so a counterfactual
//! computed here cannot be serialised into a report as though it were an observation. This is the
//! type-level form of the same rule `bioprism-oraclex` applies to reference standards, where a
//! measurement process cannot claim the deterministic rung: a simulator is a measurement process
//! whose instrument is an assumption.
//!
//! [`MechanisticModel::new`] refuses a model with an empty `known_misspecification`. A twin that
//! claims to have none is asserting it *is* the system, and the workspace's position is that such
//! a claim is not one a library should let you make by leaving a string blank.
//!
//! ## Out-of-model robustness is the deliverable, not the effect
//!
//! [`DiscrepancyProbe`] re-runs the same intervention across a caller-supplied set of alternative
//! models and reports whether the conclusion survives. A counterfactual whose *sign* flips across
//! the plausible set is not a weak oracle, it is not an oracle:
//! [`DiscrepancyProbe::is_usable_as_an_oracle`] is false and the effect magnitude is reported next
//! to the range that contradicts it. 35.04 lists out-of-model robustness as an operational metric
//! and this is the only reading of it that can fail.
//!
//! A probe over an empty alternative set is [`TwinError::NoAlternatives`], because a model compared
//! against itself is not a discrepancy measurement.
//!
//! ## The state engine is arithmetic, and it is illustrative
//!
//! [`MechanisticModel`] is a deterministic discrete-time linear compartment step: each tick moves
//! `rate[i][j]` of compartment `i`'s current mass into compartment `j`, and what is not moved
//! stays. **This crate knows no biological rate, no half-life, no growth constant and no assay
//! gain.** Every number is a caller-supplied field, and the arithmetic is the smallest thing that
//! makes an intervention and a counterfactual meaningful. It is not a model of anything; it is the
//! substrate the discrepancy machinery is demonstrated on. Anyone with a real mechanistic model
//! should implement the same [`TwinTruth`] discipline over it.
//!
//! ## What is not here
//!
//! No stochastic simulation, no parameter inference, and no posterior calibration to observed data
//! — 35.04 asks for all three, and all three are fitting procedures that need data this crate does
//! not have. No assay forward model beyond an [`AssayReadout`] that names its own transformation.
//! The workflow and cost model 35.04 also lists is `bioprism_scale::cost`'s and is not restated.
//! Nothing here reads a clock or a system RNG; the dynamics are exactly determined by the inputs.

use crate::error::TwinError;
use serde::{Deserialize, Serialize};

/// A deterministic compartment model with caller-supplied rates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MechanisticModel {
    pub id: String,
    pub compartments: Vec<String>,
    /// `rates[i][j]` is the fraction of compartment `i` moved to `j` per tick. Caller-supplied.
    pub rates: Vec<Vec<f64>>,
    /// What this model is known to get wrong. Required, and required to be non-empty.
    pub known_misspecification: String,
}

impl MechanisticModel {
    /// Builds a model, refusing one that states no known misspecification.
    pub fn new(
        id: impl Into<String>,
        compartments: Vec<String>,
        rates: Vec<Vec<f64>>,
        known_misspecification: impl Into<String>,
    ) -> Result<Self, TwinError> {
        let id = id.into();
        let known_misspecification = known_misspecification.into();
        if known_misspecification.trim().is_empty() {
            return Err(TwinError::NoStatedMisspecification(id));
        }
        if compartments.is_empty() {
            return Err(TwinError::NoCompartments { model: id });
        }
        let size = compartments.len();
        if rates.len() != size || rates.iter().any(|row| row.len() != size) {
            return Err(TwinError::RateShape {
                model: id,
                rows: rates.len(),
                cols: rates.first().map(Vec::len).unwrap_or(0),
                compartments: size,
            });
        }
        for (row, entries) in rates.iter().enumerate() {
            for (col, rate) in entries.iter().enumerate() {
                if !rate.is_finite() {
                    return Err(TwinError::NonFiniteRate {
                        model: id,
                        row,
                        col,
                    });
                }
            }
        }
        Ok(MechanisticModel {
            id,
            compartments,
            rates,
            known_misspecification,
        })
    }

    pub fn index_of(&self, compartment: &str) -> Result<usize, TwinError> {
        self.compartments
            .iter()
            .position(|name| name == compartment)
            .ok_or_else(|| TwinError::UnknownCompartment {
                model: self.id.clone(),
                compartment: compartment.to_string(),
            })
    }

    fn step(&self, state: &[f64]) -> Vec<f64> {
        let size = self.compartments.len();
        let mut next = state.to_vec();
        for (from, mass) in state.iter().enumerate().take(size) {
            for to in 0..size {
                if from == to {
                    continue;
                }
                let moved = mass * self.rates[from][to];
                next[from] -= moved;
                next[to] += moved;
            }
        }
        next
    }

    /// Runs `steps` ticks from `initial`, optionally under an intervention.
    ///
    /// An intervention is a do-operator: the named compartment is *held* at its value after every
    /// tick, rather than nudged once. Holding is what makes the contrast a causal one; a single
    /// nudge would be an initial condition.
    pub fn run(
        &self,
        initial: &[f64],
        steps: usize,
        intervention: Option<&Intervention>,
    ) -> Result<Trajectory, TwinError> {
        if initial.len() != self.compartments.len() {
            return Err(TwinError::InitialStateShape {
                model: self.id.clone(),
                given: initial.len(),
                expected: self.compartments.len(),
            });
        }
        let held = match intervention {
            Some(intervention) => Some((
                self.index_of(&intervention.compartment)?,
                intervention.hold_at,
            )),
            None => None,
        };

        let mut state = initial.to_vec();
        if let Some((index, value)) = held {
            state[index] = value;
        }
        let mut states = vec![state.clone()];
        for _ in 0..steps {
            state = self.step(&state);
            if let Some((index, value)) = held {
                state[index] = value;
            }
            states.push(state.clone());
        }
        Ok(Trajectory {
            model: self.id.clone(),
            compartments: self.compartments.clone(),
            states,
        })
    }
}

/// A do-operator: hold one compartment at a value for the whole run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Intervention {
    pub compartment: String,
    pub hold_at: f64,
}

impl Intervention {
    pub fn hold(compartment: impl Into<String>, hold_at: f64) -> Self {
        Intervention {
            compartment: compartment.into(),
            hold_at,
        }
    }
}

/// The states a run passed through, including its starting state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    pub model: String,
    pub compartments: Vec<String>,
    pub states: Vec<Vec<f64>>,
}

impl Trajectory {
    pub fn final_state(&self) -> &[f64] {
        self.states.last().map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn final_mass(&self, compartment: &str) -> Option<f64> {
        let index = self
            .compartments
            .iter()
            .position(|name| name == compartment)?;
        self.final_state().get(index).copied()
    }
}

/// A simulated readout, which names the transformation it applied.
///
/// 35.04's "assay forward model" reduced to the only part a library can honestly carry: the record
/// that a number reaching a report came out of a stated transformation of a simulated state, not
/// out of an instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssayReadout {
    pub compartment: String,
    pub value: f64,
    /// The transformation, named by the caller. This crate applies none of its own.
    pub forward_model: String,
    pub model: String,
}

impl AssayReadout {
    /// Reads `compartment` off the end of a trajectory through a caller-supplied transformation.
    ///
    /// `transform` is a plain function pointer so the readout stays deterministic and so the caller
    /// cannot smuggle in ambient state. This crate supplies no transformation of its own, not even
    /// the identity: naming it is the point.
    pub fn from_trajectory(
        trajectory: &Trajectory,
        compartment: &str,
        forward_model: impl Into<String>,
        transform: fn(f64) -> f64,
    ) -> Option<Self> {
        let mass = trajectory.final_mass(compartment)?;
        Some(AssayReadout {
            compartment: compartment.to_string(),
            value: transform(mass),
            forward_model: forward_model.into(),
            model: trajectory.model.clone(),
        })
    }
}

/// A counterfactual effect, which exists only relative to a model.
///
/// There is exactly one variant. Adding a model-free one would be the change that lets a simulated
/// answer be published as an observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "twin_truth", rename_all = "snake_case")]
pub enum TwinTruth {
    UnderModel {
        model: String,
        outcome_compartment: String,
        /// Final mass under the intervention.
        intervened: f64,
        /// Final mass without it.
        baseline: f64,
        /// `intervened - baseline`.
        effect: f64,
        known_misspecification: String,
    },
}

impl TwinTruth {
    pub fn effect(&self) -> f64 {
        match self {
            TwinTruth::UnderModel { effect, .. } => *effect,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            TwinTruth::UnderModel { model, .. } => model.as_str(),
        }
    }

    /// The sentence that must travel with the number.
    pub fn qualification(&self) -> String {
        match self {
            TwinTruth::UnderModel {
                model,
                effect,
                known_misspecification,
                ..
            } => format!(
                "effect {effect:+.4} under model {model}, which is known to get this wrong: \
                 {known_misspecification}"
            ),
        }
    }
}

/// Computes the intervened-minus-baseline contrast on one outcome compartment.
pub fn counterfactual(
    model: &MechanisticModel,
    initial: &[f64],
    steps: usize,
    intervention: &Intervention,
    outcome_compartment: &str,
) -> Result<TwinTruth, TwinError> {
    let index = model.index_of(outcome_compartment)?;
    let baseline = model.run(initial, steps, None)?;
    let intervened = model.run(initial, steps, Some(intervention))?;
    let baseline_value = baseline.final_state()[index];
    let intervened_value = intervened.final_state()[index];
    Ok(TwinTruth::UnderModel {
        model: model.id.clone(),
        outcome_compartment: outcome_compartment.to_string(),
        intervened: intervened_value,
        baseline: baseline_value,
        effect: intervened_value - baseline_value,
        known_misspecification: model.known_misspecification.clone(),
    })
}

/// The same counterfactual under a set of plausible alternative models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscrepancyProbe {
    pub reference: TwinTruth,
    pub alternatives: Vec<TwinTruth>,
    /// Every model agrees on the direction of the effect.
    pub sign_stable: bool,
    pub effect_min: f64,
    pub effect_max: f64,
    /// Widest effect minus narrowest, across the whole set including the reference.
    pub effect_span: f64,
    /// Models whose effect sign differs from the reference's.
    pub models_disagreeing: Vec<String>,
}

impl DiscrepancyProbe {
    /// Runs `intervention` under `reference` and every model in `alternatives`.
    ///
    /// Alternatives must share the reference's compartment names: two models with different state
    /// spaces produce two effects that are not the same quantity, and averaging or comparing them
    /// would be the discrepancy analysis quietly measuring nothing.
    pub fn run(
        reference: &MechanisticModel,
        alternatives: &[MechanisticModel],
        initial: &[f64],
        steps: usize,
        intervention: &Intervention,
        outcome_compartment: &str,
    ) -> Result<Self, TwinError> {
        if alternatives.is_empty() {
            return Err(TwinError::NoAlternatives);
        }
        for alternative in alternatives {
            if alternative.compartments != reference.compartments {
                return Err(TwinError::IncomparableAlternative {
                    reference: reference.id.clone(),
                    alternative: alternative.id.clone(),
                    reference_compartments: reference.compartments.clone(),
                    alternative_compartments: alternative.compartments.clone(),
                });
            }
        }

        let reference_truth =
            counterfactual(reference, initial, steps, intervention, outcome_compartment)?;
        let mut alternative_truths = Vec::new();
        for alternative in alternatives {
            alternative_truths.push(counterfactual(
                alternative,
                initial,
                steps,
                intervention,
                outcome_compartment,
            )?);
        }

        let reference_sign = sign(reference_truth.effect());
        let models_disagreeing: Vec<String> = alternative_truths
            .iter()
            .filter(|truth| sign(truth.effect()) != reference_sign)
            .map(|truth| truth.model().to_string())
            .collect();

        let effects: Vec<f64> = std::iter::once(reference_truth.effect())
            .chain(alternative_truths.iter().map(TwinTruth::effect))
            .collect();
        let effect_min = effects.iter().copied().fold(f64::INFINITY, f64::min);
        let effect_max = effects.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        Ok(DiscrepancyProbe {
            reference: reference_truth,
            alternatives: alternative_truths,
            sign_stable: models_disagreeing.is_empty(),
            effect_min,
            effect_max,
            effect_span: effect_max - effect_min,
            models_disagreeing,
        })
    }

    /// Whether the counterfactual can be used as benchmark ground truth.
    ///
    /// Only when every plausible model agrees on the direction. A sign that flips means the
    /// benchmark would grade a system on which modeller it happened to agree with.
    pub fn is_usable_as_an_oracle(&self) -> bool {
        self.sign_stable
    }

    pub fn headline(&self) -> String {
        if self.sign_stable {
            format!(
                "{} — direction holds across {} alternative model(s), effect in [{:+.4}, {:+.4}]",
                self.reference.qualification(),
                self.alternatives.len(),
                self.effect_min,
                self.effect_max
            )
        } else {
            format!(
                "{} — but the direction flips under {}; effect ranges [{:+.4}, {:+.4}] across the \
                 plausible set, so this counterfactual is not benchmark ground truth",
                self.reference.qualification(),
                self.models_disagreeing.join(", "),
                self.effect_min,
                self.effect_max
            )
        }
    }
}

/// Sign with zero as its own class, so a null effect does not silently agree with both directions.
fn sign(value: f64) -> i8 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}
