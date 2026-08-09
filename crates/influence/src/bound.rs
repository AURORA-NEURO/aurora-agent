//! What a bound is, and what it is not allowed to be.
//!
//! Blueprint 43.28's runtime contract asks an influence bound to carry "metric, method, confidence,
//! and validity scope". Three of those four are here as typed fields. *Confidence* is deliberately
//! absent: every method in this crate is a deterministic worst-case argument, not a sampling
//! procedure, so the only honest confidence is one, and a field that always reads one invites a
//! future method to fill it with something softer while the certificate still says `Bounded`.
//! A probabilistic bound would need a different type, and it does not exist yet.
//!
//! ## The invariant made unrepresentable
//!
//! `AGENTS.md` asks that rules be enforced by types rather than by comments. The rule here is that
//! *a bound that could not be computed is not a large bound*. [`InfluenceBound`] has private
//! fields and one fallible constructor that rejects anything outside `[0, 1]` — infinity and NaN
//! included — so there is no value of the type that means "unbounded". The absence of a bound is
//! [`InfluenceEstimate::Unknown`], a different variant carrying a different payload, and the
//! conversion into a [`bioprism_section::OmissionGroup`] maps it to
//! [`bioprism_section::InfluenceClass::Unknown`] with `bound: None`.

use crate::error::{InfluenceError, UnknownReason};
use serde::{Deserialize, Serialize};

/// The metric a bound is stated in.
///
/// One variant, and it is an enum anyway: a bare `f64` on a certificate is unreadable without
/// knowing what it measures, and the natural second metric — a bound on the change in a decision
/// *loss* rather than in the answer distribution — needs the decision loss that `fiber-query/0.1`
/// does not declare. See [`crate::NOT_IMPLEMENTED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfluenceMetric {
    /// Total-variation distance between the normalised answers over the query's free variables.
    ///
    /// Normalisation is what makes the number comparable across queries and composable along a
    /// path. It also fixes the interpretation: a bound of `ε` says no permitted action whose
    /// decision boundary sits further than `ε` from the computed answer can flip.
    TotalVariationOnNormalisedAnswer,
}

impl InfluenceMetric {
    pub fn as_str(self) -> &'static str {
        match self {
            InfluenceMetric::TotalVariationOnNormalisedAnswer => {
                "total_variation_on_normalised_answer"
            }
        }
    }
}

/// Which argument produced the number.
///
/// The method is on the bound rather than in a log because tightness is a property of the method,
/// and a reader comparing two certificates needs to know whether a larger number means more
/// influence or a cheaper argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundMethod {
    /// The region was executed with and without the factor and the answers compared. Exact.
    ExactRemoval,
    /// The lemma of [`crate::ratio`] applied to the perturbed factor's own dynamic range.
    /// Structural: touches no other factor and runs no query.
    DynamicRange,
    /// Ratio ranges composed across a group, then the lemma. See [`crate::ratio::RatioRange::compose`].
    RatioComposition,
    /// Per-step Dobrushin coefficients multiplied along the dependency path. See
    /// [`crate::contraction`].
    ChainContraction,
    /// No dependency path reaches the target, so the bound is exactly zero. This is
    /// [`bioprism_section::InfluenceClass::Zero`] arrived at from the numeric side, and it is kept
    /// distinct from a computed `0.0` because the arguments differ.
    StructuralZero,
}

impl BoundMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            BoundMethod::ExactRemoval => "exact_removal",
            BoundMethod::DynamicRange => "dynamic_range",
            BoundMethod::RatioComposition => "ratio_composition",
            BoundMethod::ChainContraction => "chain_contraction",
            BoundMethod::StructuralZero => "structural_zero",
        }
    }

    /// Whether this method's number is the true influence rather than an upper bound on it.
    pub fn is_exact(self) -> bool {
        matches!(self, BoundMethod::ExactRemoval | BoundMethod::StructuralZero)
    }
}

/// Which way the approximation errs.
///
/// The task specification asks that the direction be stated in the docs. It is stated in the type
/// instead, because a doc comment is not carried on a certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Approximation {
    /// The reported value equals the true maximum influence.
    Exact,
    /// The reported value is at least the true maximum influence. Never below it.
    ConservativeUpperBound,
}

/// A number with the provenance that makes it a guarantee.
///
/// Constructed only through [`InfluenceBound::new`], which enforces `value ∈ [0, 1]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfluenceBound {
    value: f64,
    metric: InfluenceMetric,
    method: BoundMethod,
    approximation: Approximation,
    /// What the bound is a statement about: which region, which perturbation, which factors.
    /// 43.28 requires bounds to be "query- and scope-specific" and this is where that is recorded.
    validity: String,
}

impl InfluenceBound {
    pub fn new(
        value: f64,
        metric: InfluenceMetric,
        method: BoundMethod,
        approximation: Approximation,
        validity: impl Into<String>,
    ) -> Result<Self, InfluenceError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(InfluenceError::BoundOutOfRange { value });
        }
        Ok(InfluenceBound {
            value,
            metric,
            method,
            approximation,
            validity: validity.into(),
        })
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn metric(&self) -> InfluenceMetric {
        self.metric
    }

    pub fn method(&self) -> BoundMethod {
        self.method
    }

    pub fn approximation(&self) -> Approximation {
        self.approximation
    }

    pub fn validity(&self) -> &str {
        &self.validity
    }

    /// A bound of one. Sound, and it excludes nothing.
    ///
    /// A vacuous bound still beats [`InfluenceEstimate::Unknown`] on one axis — it is a
    /// commitment, so a soundness test can falsify it — and loses on another: it licenses no
    /// sufficiency claim a reader would want. Certificates should say so rather than quietly
    /// counting it as a win, which is why [`crate::manifest`] reports vacuous groups separately.
    pub fn is_vacuous(&self) -> bool {
        self.value >= 1.0
    }

    /// The smaller of two sound bounds on the same quantity, which is again sound.
    ///
    /// Ties keep `self`, so a caller that orders methods by preference gets a deterministic
    /// winner rather than one decided by float comparison.
    pub fn tightest(self, other: InfluenceBound) -> InfluenceBound {
        if other.value < self.value {
            other
        } else {
            self
        }
    }
}

/// The result of asking for a bound: a number with provenance, or a named reason there is none.
///
/// This enum is the crate's whole contract with `bioprism-section`. There is no third state and no
/// numeric encoding of the second one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum InfluenceEstimate {
    Bounded(InfluenceBound),
    Unknown(UnknownReason),
}

impl InfluenceEstimate {
    pub fn bound(&self) -> Option<&InfluenceBound> {
        match self {
            InfluenceEstimate::Bounded(bound) => Some(bound),
            InfluenceEstimate::Unknown(_) => None,
        }
    }

    pub fn unknown_reason(&self) -> Option<&UnknownReason> {
        match self {
            InfluenceEstimate::Bounded(_) => None,
            InfluenceEstimate::Unknown(reason) => Some(reason),
        }
    }

    pub fn is_bounded(&self) -> bool {
        matches!(self, InfluenceEstimate::Bounded(_))
    }

    /// Whether a group carrying this estimate may contribute to a sufficiency claim.
    ///
    /// Mirrors [`bioprism_section::InfluenceClass::supports_sufficiency`] exactly. A vacuous bound
    /// still supports sufficiency *formally* — the class is `Bounded` and the manifest is not
    /// voided — which is correct and is also why `is_vacuous` is public: a bound of one on an
    /// answer that lives in `[0, 1]` constrains nothing, and a consumer that treats every
    /// `Bounded` group as reassuring will be reassured by nothing.
    pub fn supports_sufficiency(&self) -> bool {
        self.is_bounded()
    }
}
