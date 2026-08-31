//! The reference standard as a distribution, not a label (31.01).
//!
//! This is the module the rest of the crate is built around. 31.01's claim is that reference
//! truth in biology is "distributions, intervals, sets, and partial orders whenever the biology
//! or measurement does not justify one categorical answer", and its worked case is blunt: a
//! calibrated `0.55 / 0.35 / 0.10` forecast "can be more correct than an unqualified categorical
//! label". An evaluation harness that stores `label: "progression"` has already thrown away the
//! information that would have made that judgement possible.
//!
//! Two consequences run through every other module:
//!
//! 1. **A categorical answer cannot score 1.0 against a distributed reference.** The most a
//!    single-state answer can collect is the mass the reference places on that state, because
//!    that is all the reference is willing to assert. This is not a penalty applied to the
//!    system; it is the ceiling of what the instrument can measure. See
//!    [`ReferenceDistribution::attainable_ceiling`].
//! 2. **Spread has to be attributed before it can be discharged.** 31.01 requires separating
//!    "aleatoric disagreement from annotation error". They look identical in the mass vector and
//!    they mean opposite things: aleatoric spread is the biology, annotation spread is a defect
//!    in the reference standard that better adjudication would remove. [`Dispersion`] makes the
//!    attribution explicit and [`Dispersion::Unattributed`] makes *not having done it* explicit.
//!
//! # Not implemented
//!
//! 31.01 also asks for partial orders over taxonomic and causal graphs, and for "distance on
//! taxonomic or causal graphs" as a metric. This module represents only flat mass over named
//! states and interval-valued references; a subtype hierarchy in which `GBM, IDH-wildtype` is a
//! near miss for `astrocytoma, IDH-mutant` but a far miss for `metastasis` needs an ontology,
//! which lives outside this crate. Until then such near-misses must be expressed as reference
//! mass on both states, not as a graph distance.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::ReferenceError;
use crate::validation::valid_text;

/// Masses are checked against 1.0 to this tolerance.
///
/// Wide enough that a hand-written `0.55 / 0.35 / 0.10` passes, narrow enough that a vector
/// summing to 0.9 — which would inflate every mass lookup — does not.
pub const MASS_TOLERANCE: f64 = 1e-9;

/// Where the spread in a reference standard comes from (31.01, "separate aleatoric disagreement
/// from annotation error").
///
/// The distinction decides what the spread licenses. Irreducible biological variation means a
/// confident categorical answer is *wrong to give*; annotation error means the reference itself
/// is broken and the evaluation is measuring the annotators.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Dispersion {
    /// Genuine biological or measurement variability. A spatially mixed tumour really is two
    /// things at once, and no amount of re-reading collapses it.
    Aleatoric,
    /// Reader or curation noise. Better adjudication would shrink this, so a system penalised by
    /// it is being penalised for the benchmark's defect.
    AnnotationError,
    /// Both present, with the share attributable to irreducible biology stated.
    Mixed { aleatoric_fraction: f64 },
    /// Nobody attributed the spread. Representable, and it never supports a clean pass or a
    /// midpoint collapse.
    Unattributed,
}

impl Dispersion {
    /// Whether the source of the spread has actually been analysed.
    pub fn is_attributed(self) -> bool {
        !matches!(self, Dispersion::Unattributed)
    }

    /// The share of the spread that no better reference standard could remove.
    ///
    /// [`Dispersion::Unattributed`] returns `None` rather than a guess: the whole point of the
    /// variant is that the number is not known.
    pub fn irreducible_fraction(self) -> Option<f64> {
        match self {
            Dispersion::Aleatoric => Some(1.0),
            Dispersion::AnnotationError => Some(0.0),
            Dispersion::Mixed { aleatoric_fraction } => Some(aleatoric_fraction),
            Dispersion::Unattributed => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Dispersion::Aleatoric => "aleatoric",
            Dispersion::AnnotationError => "annotation_error",
            Dispersion::Mixed { .. } => "mixed",
            Dispersion::Unattributed => "unattributed",
        }
    }
}

/// How sharp the reference standard is.
///
/// Derived from the mass vector rather than stored, so it cannot drift out of agreement with it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "resolution", rename_all = "snake_case")]
pub enum Resolution {
    /// Exactly one admissible state at full mass. The only shape against which a clean pass is
    /// even definable.
    Categorical,
    /// Mass is spread across several states. `modal_mass` is the largest single mass, and it is
    /// the ceiling for any unqualified categorical answer.
    Distributed { modal_mass: f64 },
}

impl Resolution {
    pub fn is_categorical(self) -> bool {
        matches!(self, Resolution::Categorical)
    }

    /// The largest mass the reference places on any single state.
    pub fn modal_mass(self) -> f64 {
        match self {
            Resolution::Categorical => 1.0,
            Resolution::Distributed { modal_mass } => modal_mass,
        }
    }
}

/// A normalised distribution over the states a reference standard admits.
///
/// Constructed only through [`ReferenceDistribution::new`], which validates normalisation. There
/// is no unchecked constructor: a mass vector summing to 0.8 makes every subsequent lookup read
/// low and every Brier score read wrong, and the failure is silent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceDistribution {
    mass: BTreeMap<String, f64>,
    dispersion: Dispersion,
}

impl ReferenceDistribution {
    /// Builds a reference distribution, rejecting anything that is not a distribution.
    ///
    /// Masses are *not* renormalised. Rescaling a vector that sums to 0.8 would silently promote
    /// every state by 25% and turn an under-specified reference into a confident one.
    pub fn new(
        mass: impl IntoIterator<Item = (String, f64)>,
        dispersion: Dispersion,
    ) -> Result<Self, ReferenceError> {
        let mut table: BTreeMap<String, f64> = BTreeMap::new();
        for (state, m) in mass {
            if table.contains_key(&state) {
                return Err(ReferenceError::DuplicateState { state });
            }
            table.insert(state, m);
        }
        let reference = ReferenceDistribution {
            mass: table,
            dispersion,
        };
        reference.validate()?;
        Ok(reference)
    }

    /// Re-check a deserialized or caller-retained distribution before any scoring operation.
    ///
    /// `Deserialize` is intentionally available for persistence, so consumers must not assume
    /// that the constructor was the only path by which a reference entered memory.
    pub(crate) fn validate(&self) -> Result<(), ReferenceError> {
        if let Dispersion::Mixed { aleatoric_fraction } = self.dispersion {
            if !(0.0..=1.0).contains(&aleatoric_fraction) || !aleatoric_fraction.is_finite() {
                return Err(ReferenceError::AleatoricFractionOutOfRange {
                    fraction: aleatoric_fraction,
                });
            }
        }
        if self.mass.is_empty() {
            return Err(ReferenceError::NoAdmissibleState);
        }
        for (state, mass) in &self.mass {
            if !valid_text(state) {
                return Err(ReferenceError::InvalidState {
                    state: state.clone(),
                });
            }
            if !mass.is_finite() {
                return Err(ReferenceError::NonFiniteMass {
                    state: state.clone(),
                });
            }
            if *mass < 0.0 {
                return Err(ReferenceError::NegativeMass {
                    state: state.clone(),
                    mass: *mass,
                });
            }
        }
        let total: f64 = self.mass.values().sum();
        if !total.is_finite() || (total - 1.0).abs() > MASS_TOLERANCE {
            return Err(ReferenceError::MassNotNormalised {
                total,
                tolerance: MASS_TOLERANCE,
            });
        }
        Ok(())
    }

    /// A reference standard that really does admit one answer: mass 1.0 on a single state.
    ///
    /// Kept as a named constructor so that the categorical case is visibly the special case and
    /// not the default shape.
    pub fn resolved(state: impl Into<String>) -> Self {
        ReferenceDistribution {
            mass: BTreeMap::from([(state.into(), 1.0)]),
            dispersion: Dispersion::Aleatoric,
        }
    }

    pub fn dispersion(&self) -> Dispersion {
        self.dispersion
    }

    pub fn states(&self) -> impl Iterator<Item = &str> {
        self.mass.keys().map(String::as_str)
    }

    pub fn admits(&self, state: &str) -> bool {
        self.mass.contains_key(state)
    }

    /// The mass on `state`, or `None` when the reference never enumerated it.
    ///
    /// `None` rather than `0.0`, because the two mean different things: mass zero is the
    /// reference asserting the state is impossible, absence is the reference never having
    /// considered it. Scoring conflates them at its peril, so [`crate::score`] refuses instead.
    pub fn mass_on(&self, state: &str) -> Option<f64> {
        self.mass.get(state).copied()
    }

    /// The state carrying the most mass, with that mass.
    ///
    /// Ties resolve to the lexicographically first state, which is arbitrary but deterministic;
    /// callers that care about ties should read [`ReferenceDistribution::is_modally_tied`].
    pub fn mode(&self) -> (&str, f64) {
        let mut best: Option<(&str, f64)> = None;
        for (state, &m) in &self.mass {
            match best {
                Some((_, best_mass)) if m <= best_mass => {}
                _ => best = Some((state.as_str(), m)),
            }
        }
        best.expect("a reference distribution always has at least one state")
    }

    /// Whether more than one state shares the modal mass, in which case naming "the" reference
    /// answer is a coin flip dressed as a label.
    pub fn is_modally_tied(&self) -> bool {
        let (_, top) = self.mode();
        self.mass
            .values()
            .filter(|&&m| (m - top).abs() <= MASS_TOLERANCE)
            .count()
            > 1
    }

    /// Whether the reference actually decides the case.
    ///
    /// Counts states carrying mass, not states enumerated. A reference that lists three
    /// possibilities and puts all the mass on one has decided; listing the alternatives it ruled
    /// out is good practice and must not be punished as if it were hedging.
    pub fn resolution(&self) -> Resolution {
        if self.mass.values().filter(|&&m| m > 0.0).count() == 1 {
            Resolution::Categorical
        } else {
            Resolution::Distributed {
                modal_mass: self.mode().1,
            }
        }
    }

    /// How confident the reference standard is in its own best answer.
    ///
    /// 1.0 for a resolved reference, the modal mass otherwise. This is the quantity a publication
    /// policy gates on: a benchmark whose references rarely clear 0.5 is measuring its annotators,
    /// and [`crate::score::CollapsePolicy::minimum_reference_confidence`] refuses to emit a number
    /// for such a case. It is deliberately *not* the score ceiling — see [`crate::score`], where
    /// both readings of the spread are carried instead of one being picked here.
    pub fn modal_confidence(&self) -> f64 {
        self.mode().1
    }

    /// Shannon entropy in bits: how undecided the reference is, independent of how many states
    /// it enumerates.
    pub fn entropy_bits(&self) -> f64 {
        self.mass
            .values()
            .filter(|&&m| m > 0.0)
            .map(|&m| -m * m.log2())
            .sum()
    }
}

/// What a reference standard says about one case.
///
/// 31.01 lets an oracle answer `supported`, `contradicted`, `unresolved`, `not-evaluable`, or a
/// structured distribution. The first two are distributions with two states; the last three are
/// distinct shapes, and collapsing them is how "the oracle declined" becomes "the system was
/// wrong".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "standard", rename_all = "snake_case")]
pub enum ReferenceStandard {
    /// The reference asserts a distribution over admissible answers.
    Distribution(ReferenceDistribution),
    /// The reference standard applies here and cannot decide. 31.01: "`Unresolved` is not scored
    /// as a hidden negative unless the task explicitly requires an action under uncertainty."
    Unresolved { reason: String },
    /// The case is outside the reference standard's declared scope. Distinct from unresolved:
    /// the instrument was never pointed at this, so the absence of an answer says nothing about
    /// the difficulty of the case.
    NotEvaluable { reason: String },
}

impl ReferenceStandard {
    pub fn distribution(&self) -> Option<&ReferenceDistribution> {
        match self {
            ReferenceStandard::Distribution(d) => Some(d),
            _ => None,
        }
    }

    /// Whether this reference standard could, in principle, certify a prediction as fully
    /// correct. False for every distributed, unresolved or out-of-scope reference.
    pub fn can_certify_a_clean_pass(&self) -> bool {
        matches!(self, ReferenceStandard::Distribution(d) if d.resolution().is_categorical())
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ReferenceStandard::Distribution(_) => "distribution",
            ReferenceStandard::Unresolved { .. } => "unresolved",
            ReferenceStandard::NotEvaluable { .. } => "not_evaluable",
        }
    }
}
