//! The interval domain over pointwise reweightings — 43.11 applied to [`crate::ratio`]'s lemma.
//!
//! This is the domain the crate already had, promoted from a struct with one transformer into a
//! member of a lattice. [`crate::ratio::RatioRange`] was and remains the *value* the lemma consumes;
//! [`RatioInterval`] adds the two things a value is not: a `⊥` distinguishable from the interval
//! `[0, 0]`, and the join, meet and widening that let a fixed-point solver work in it.
//!
//! ## The concretisation
//!
//! ```text
//!     γ(⊥)          =  ∅
//!     γ([lo, hi])   =  { w ∈ ℝ , 0 ≤ w < ∞ : lo ≤ w ≤ hi }
//! ```
//!
//! A concrete element is one factor of the pointwise reweighting the perturbation applies to the
//! joint measure — one number per joint assignment. `⊤ = [0, ∞]` therefore says "this perturbation
//! may do anything to any assignment", which is exactly the state a factor with no declared
//! potential is in.
//!
//! ## The transformers, each with the inclusion it satisfies
//!
//! | `f#` | concrete `f` | soundness |
//! |---|---|---|
//! | [`multiply`] | `(v, w) ↦ v·w` | `γ(a)·γ(b) ⊆ γ(multiply(a, b))` |
//! | [`reciprocal`] | `w ↦ 1/w` | `{1/w : w ∈ γ(a), w > 0} ⊆ γ(reciprocal(a))` |
//! | [`abstract_removal_of`] | the reweighting `1/φ` a removal induces | `α` of a concrete table |
//! | [`RatioInterval::total_variation_bound`] | `TV(p, p')` | the lemma of [`crate::ratio`] |
//!
//! The last row is not a transformer into this domain but an *observation* out of it, and it is
//! sound in the same sense: every reweighting in `γ(a)` moves a normalised answer by at most the
//! reported number. It is monotone in the interval order, which is what lets it be read off a
//! widened element without a separate argument.
//!
//! ## Why widening here is the textbook one and not a threshold ladder
//!
//! No fixed point in this crate is computed in this domain — reweightings compose along a *finite*
//! product of perturbed factors, so the ascending chains that need widening live in
//! [`crate::domains::Displacement`]. The classical `0 / ∞` widening is therefore enough, and it
//! terminates for the classical reason: an endpoint that moves at all moves to an extreme, so each
//! of the two endpoints changes at most twice in any widening sequence.

use crate::domain::{AbstractDomain, DomainId, EnumerableConcretisation, FactClass};
use crate::error::InfluenceError;
use crate::ratio::RatioRange;
use serde::{Deserialize, Serialize};

/// The registered name of [`RatioIntervalDomain`].
pub const RATIO_INTERVAL_DOMAIN: &str = "ratio_interval";

/// An interval of admissible pointwise reweightings, or `⊥`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "element")]
pub enum RatioInterval {
    /// `γ = ∅`. Distinct from `[0, 0]`, whose concretisation is the single reweighting that
    /// annihilates the measure.
    Bottom,
    /// `0 ≤ lo ≤ hi ≤ ∞`.
    Range { lo: f64, hi: f64 },
}

impl RatioInterval {
    /// Rejects an inverted or negative interval. `hi` may be infinite; `lo` may not.
    pub fn range(lo: f64, hi: f64) -> Result<Self, InfluenceError> {
        if !lo.is_finite() || lo < 0.0 {
            return Err(InfluenceError::InadmissibleRatioEndpoint { value: lo });
        }
        if hi.is_nan() || hi < 0.0 {
            return Err(InfluenceError::InadmissibleRatioEndpoint { value: hi });
        }
        if lo > hi {
            return Err(InfluenceError::InvertedRatioRange { lo, hi });
        }
        Ok(RatioInterval::Range { lo, hi })
    }

    /// The identity reweighting: the perturbation does nothing.
    pub fn identity() -> Self {
        RatioInterval::Range { lo: 1.0, hi: 1.0 }
    }

    pub fn endpoints(self) -> Option<(f64, f64)> {
        match self {
            RatioInterval::Bottom => None,
            RatioInterval::Range { lo, hi } => Some((lo, hi)),
        }
    }

    /// Whether the only total-variation bound this element supports is the vacuous one.
    pub fn is_vacuous(self) -> bool {
        match self {
            RatioInterval::Bottom => false,
            RatioInterval::Range { lo, hi } => hi > 0.0 && (lo == 0.0 || !hi.is_finite()),
        }
    }

    /// The lemma of [`crate::ratio`], read off this element.
    ///
    /// `⊥` gives `0.0`: no reweighting concretises to it, so every reweighting in `γ(⊥)` moves the
    /// answer by at most zero, vacuously. An analysis that reaches `⊥` has proved the case
    /// unreachable and the number is not a claim about a reachable one.
    ///
    /// The value is monotone in the interval order — widening an element can only widen the ratio
    /// `hi/lo` the lemma depends on — so reading it off a post-fixpoint needs no separate argument.
    pub fn total_variation_bound(self) -> f64 {
        match self {
            RatioInterval::Bottom => 0.0,
            RatioInterval::Range { lo, hi } => match RatioRange::new(lo, hi) {
                Ok(range) => range.total_variation_bound(),
                Err(_) => RatioRange::vacuous().total_variation_bound(),
            },
        }
    }
}

/// `γ(a)·γ(b) ⊆ γ(multiply(a, b))`.
///
/// Interval multiplication restricted to non-negative endpoints, which is monotone in both
/// arguments, so the extremes are attained at the endpoint pairs. The one case IEEE arithmetic gets
/// wrong is `0 · ∞`, which it calls NaN and which is here the product of an interval pinned at zero
/// with an unbounded one: every concrete product is `0 · w = 0`, so the sound answer is `0`.
pub fn multiply(left: RatioInterval, right: RatioInterval) -> RatioInterval {
    let (Some((llo, lhi)), Some((rlo, rhi))) = (left.endpoints(), right.endpoints()) else {
        return RatioInterval::Bottom;
    };
    RatioInterval::Range {
        lo: llo * rlo,
        hi: extended_product(lhi, rhi),
    }
}

fn extended_product(left: f64, right: f64) -> f64 {
    if left == 0.0 || right == 0.0 {
        0.0
    } else {
        left * right
    }
}

/// `{1/w : w ∈ γ(a), w > 0} ⊆ γ(reciprocal(a))`.
///
/// `1/0 = ∞` and `1/∞ = 0` by the limit, which is why removing a factor with a zero entry produces
/// an unbounded interval rather than an error: the reweighting really is unbounded there.
pub fn reciprocal(element: RatioInterval) -> RatioInterval {
    match element {
        RatioInterval::Bottom => RatioInterval::Bottom,
        RatioInterval::Range { lo, hi } => RatioInterval::Range {
            lo: if hi.is_finite() { 1.0 / hi } else { 0.0 },
            hi: if lo == 0.0 { f64::INFINITY } else { 1.0 / lo },
        },
    }
}

/// `α` of a factor's potential under [`crate::Perturbation::Removal`].
///
/// Removal reweights the joint measure by `1/φ`, so this is [`reciprocal`] of the interval hull of
/// the table's entries. An empty table, or one that is identically zero, contributes no reweighting
/// at all and abstracts to the identity.
pub fn abstract_removal_of(table: &[f64]) -> Result<RatioInterval, InfluenceError> {
    let mut lo = f64::INFINITY;
    let mut hi = 0.0f64;
    for entry in table {
        if !entry.is_finite() || *entry < 0.0 {
            return Err(InfluenceError::InadmissibleRatioEndpoint { value: *entry });
        }
        lo = lo.min(*entry);
        hi = hi.max(*entry);
    }
    if table.is_empty() || hi == 0.0 {
        return Ok(RatioInterval::identity());
    }
    Ok(reciprocal(RatioInterval::Range { lo, hi }))
}

/// The interval lattice over pointwise reweightings of the joint measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RatioIntervalDomain;

impl AbstractDomain for RatioIntervalDomain {
    type Element = RatioInterval;
    type Concrete = f64;

    fn id(&self) -> DomainId {
        DomainId::new(RATIO_INTERVAL_DOMAIN)
    }

    fn abstracts(&self) -> FactClass {
        FactClass::JointReweighting
    }

    fn bottom(&self) -> RatioInterval {
        RatioInterval::Bottom
    }

    fn top(&self) -> RatioInterval {
        RatioInterval::Range {
            lo: 0.0,
            hi: f64::INFINITY,
        }
    }

    fn leq(&self, left: &RatioInterval, right: &RatioInterval) -> bool {
        match (left.endpoints(), right.endpoints()) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some((llo, lhi)), Some((rlo, rhi))) => rlo <= llo && lhi <= rhi,
        }
    }

    fn join(&self, left: &RatioInterval, right: &RatioInterval) -> RatioInterval {
        match (left.endpoints(), right.endpoints()) {
            (None, _) => *right,
            (_, None) => *left,
            (Some((llo, lhi)), Some((rlo, rhi))) => RatioInterval::Range {
                lo: llo.min(rlo),
                hi: lhi.max(rhi),
            },
        }
    }

    fn meet(&self, left: &RatioInterval, right: &RatioInterval) -> RatioInterval {
        match (left.endpoints(), right.endpoints()) {
            (None, _) | (_, None) => RatioInterval::Bottom,
            (Some((llo, lhi)), Some((rlo, rhi))) => {
                let lo = llo.max(rlo);
                let hi = lhi.min(rhi);
                if lo > hi {
                    RatioInterval::Bottom
                } else {
                    RatioInterval::Range { lo, hi }
                }
            }
        }
    }

    fn widen(&self, previous: &RatioInterval, next: &RatioInterval) -> RatioInterval {
        match (previous.endpoints(), next.endpoints()) {
            (None, _) => *next,
            (_, None) => *previous,
            (Some((plo, phi)), Some((nlo, nhi))) => RatioInterval::Range {
                lo: if nlo < plo { 0.0 } else { plo },
                hi: if nhi > phi { f64::INFINITY } else { phi },
            },
        }
    }

    fn concretises(&self, element: &RatioInterval, concrete: &f64) -> bool {
        match element.endpoints() {
            None => false,
            Some((lo, hi)) => concrete.is_finite() && *concrete >= lo && *concrete <= hi,
        }
    }

    fn render(&self, element: &RatioInterval) -> String {
        match element.endpoints() {
            None => "⊥".to_string(),
            Some((lo, hi)) => format!("[{lo}, {hi}]"),
        }
    }
}

impl EnumerableConcretisation for RatioIntervalDomain {
    /// A dyadic grid through `[0, 16]` together with the endpoints the lattice singles out.
    ///
    /// The concrete class is the non-negative reals, so this is a *grid*, not the class. Every law
    /// checked over it is a falsification search in exactly the sense
    /// [`crate::bruteforce`] uses for the multiplicative-range perturbation: a failure is a
    /// counterexample, a pass is evidence.
    fn concrete_universe(&self) -> Vec<f64> {
        let mut universe = vec![0.0];
        let mut value = 0.0625f64;
        while value <= 16.0 {
            universe.push(value);
            universe.push(value * 1.5);
            value *= 2.0;
        }
        universe
    }

    fn universe_is_complete(&self) -> bool {
        false
    }
}
