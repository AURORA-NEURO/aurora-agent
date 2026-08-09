//! The one inequality this crate is built on.
//!
//! Blueprint 43.28 requires the compiler to "compute exact or conservative influence bounds" and
//! to "propagate bounds through the decision algebra". It names neither the metric nor the method,
//! and it gives no composition rule. Both are decisions of this implementation and are stated here
//! rather than presented as specification.
//!
//! ## The lemma
//!
//! Let `p` be a probability distribution and let a perturbation reweight it pointwise,
//! `p'(x) ∝ p(x)·w(x)`, with `w(x) ∈ [a, b]` and `0 < a ≤ b < ∞`. Then
//!
//! ```text
//!     TV(p, p')  ≤  (√b − √a) / (√b + √a)
//! ```
//!
//! and the inequality is attained, so no uniformly better bound exists from the ratio range alone.
//!
//! *Proof.* Write `m = E_p[w]`, so `p'(x) = p(x)w(x)/m` and
//! `TV(p, p') = ½ Σ_x p(x)|w(x)/m − 1| = (1/2m)·E_p|w − m|`, the mean absolute deviation of `w`
//! scaled by `1/2m`. Over all distributions of `w` supported on `[a, b]`, mean absolute deviation
//! at a given mean is maximised by a two-point law on the endpoints, so it suffices to maximise
//! over `w ∈ {a, b}` with mass `q` on `b`. Substituting `t = q(b − a)` gives
//! `TV = t(b − a − t) / ((b − a)(a + t))`, whose derivative vanishes at `t* = √(ab) − a`.
//! Evaluating there, the `√(ab)` factors cancel and the value is
//! `(√b − √a)² / (b − a) = (√b − √a)/(√b + √a)`. ∎
//!
//! Three properties make this the right primitive rather than the more obvious `(b − a)/(2a)`:
//!
//! - It never exceeds one, so it is always a statement about a total-variation distance. The naive
//!   bound exceeds one as soon as `b > 3a`, at which point it is not a bound, it is a number.
//! - It depends only on the *ratio* `b/a`, so it is invariant to rescaling the perturbed factor —
//!   which is correct, because a global rescale of one factor cannot change a normalised answer.
//! - It composes: perturbing two factors independently multiplies their ratio ranges, so the
//!   composed spread is the product of the spreads. See [`RatioRange::compose`].
//!
//! ## The direction of every approximation here
//!
//! Upward. The lemma bounds the worst case over *all* base distributions `p`, and a real region
//! fixes one. It bounds the worst case over all `w` consistent with the range, and a real
//! perturbation picks one. Both slacks push the reported number above the truth, never below.
//! [`crate::exact`] closes both of them for the one case where they can be closed exactly.

use crate::error::InfluenceError;
use serde::{Deserialize, Serialize};

/// An interval the pointwise reweighting of the joint measure is known to stay inside.
///
/// Endpoints are ratios, not values: `[1, 1]` is the identity perturbation and `[0, ∞)` is not
/// representable, because an unbounded reweighting is exactly the case where nothing can be said
/// and the crate reports [`crate::UnknownReason`] instead of a number.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RatioRange {
    lo: f64,
    hi: f64,
}

impl RatioRange {
    pub fn new(lo: f64, hi: f64) -> Result<Self, InfluenceError> {
        for value in [lo, hi] {
            if !value.is_finite() || value < 0.0 {
                return Err(InfluenceError::InadmissibleRatioEndpoint { value });
            }
        }
        if lo > hi {
            return Err(InfluenceError::InvertedRatioRange { lo, hi });
        }
        Ok(RatioRange { lo, hi })
    }

    /// The identity perturbation: the factor is left exactly as it is.
    pub fn identity() -> Self {
        RatioRange { lo: 1.0, hi: 1.0 }
    }

    /// The range `[0, ∞)`, whose only sound bound is one.
    ///
    /// [`RatioRange::new`] rejects a non-finite endpoint because an infinite ratio arriving
    /// through a caller's arithmetic is a bug rather than an intention. Removing a factor with a
    /// zero entry produces one legitimately — the reciprocal of zero — so it gets a named
    /// constructor instead of an error. The distinction matters: this is still a `Bounded`
    /// estimate, it is simply bounded by the largest value the metric has, and
    /// [`crate::InfluenceBound::is_vacuous`] is how a reader tells that apart from a useful bound.
    pub fn vacuous() -> Self {
        RatioRange {
            lo: 0.0,
            hi: f64::INFINITY,
        }
    }

    /// Whether this range can only support the vacuous bound.
    pub fn is_vacuous(self) -> bool {
        self.lo == 0.0 || !self.hi.is_finite()
    }

    /// The ratio range induced by replacing a factor with the all-ones factor.
    ///
    /// Removal reweights the joint measure by `1/φ`, so the range is the reciprocal of the
    /// factor's own range. A factor with a zero entry gives `lo = 0`: removing it can move an
    /// impossible assignment to a possible one, which no ratio can bound, and the resulting
    /// total-variation bound is the vacuous `1.0`. That is a sound bound and a useless one, and
    /// [`crate::InfluenceBound::is_vacuous`] lets a caller tell the difference.
    pub fn of_removal(table: &[f64]) -> Result<Self, InfluenceError> {
        let mut min = f64::INFINITY;
        let mut max = 0.0f64;
        for entry in table {
            if !entry.is_finite() || *entry < 0.0 {
                return Err(InfluenceError::InadmissibleRatioEndpoint { value: *entry });
            }
            min = min.min(*entry);
            max = max.max(*entry);
        }
        if table.is_empty() || max == 0.0 {
            return Ok(RatioRange::identity());
        }
        if min == 0.0 {
            return Ok(RatioRange::vacuous());
        }
        RatioRange::new(1.0 / max, 1.0 / min)
    }

    pub fn lo(self) -> f64 {
        self.lo
    }

    pub fn hi(self) -> f64 {
        self.hi
    }

    /// `hi / lo`, the quantity the bound actually depends on. Infinite when the range is vacuous.
    pub fn spread(self) -> f64 {
        if self.lo == 0.0 && self.hi == 0.0 {
            1.0
        } else if self.is_vacuous() {
            f64::INFINITY
        } else {
            self.hi / self.lo
        }
    }

    /// Two independent perturbations applied together.
    ///
    /// The joint reweighting is the product of the individual reweightings, so the ranges
    /// multiply. This is the composition rule 43.28 asks for and does not supply. It is
    /// conservative in the same direction as the lemma: it assumes the two perturbations can
    /// conspire to hit both extremes on the same assignment, which a real pair of factors sharing
    /// no scope generally cannot.
    pub fn compose(self, other: RatioRange) -> RatioRange {
        if self.is_vacuous() || other.is_vacuous() {
            return RatioRange::vacuous();
        }
        RatioRange {
            lo: self.lo * other.lo,
            hi: self.hi * other.hi,
        }
    }

    /// `(√hi − √lo) / (√hi + √lo)`, or `1.0` when the range reaches zero.
    ///
    /// This is the whole crate in one line. Everything else decides *which* range to hand it and
    /// how to compose several of them.
    pub fn total_variation_bound(self) -> f64 {
        if self.is_vacuous() {
            return if self.hi == 0.0 { 0.0 } else { 1.0 };
        }
        if self.hi == self.lo {
            return 0.0;
        }
        let root_lo = self.lo.sqrt();
        let root_hi = self.hi.sqrt();
        ((root_hi - root_lo) / (root_hi + root_lo)).clamp(0.0, 1.0)
    }
}

/// The triangle-inequality alternative to [`RatioRange::compose`].
///
/// Total variation is a metric, so perturbing a group one factor at a time gives a chain and the
/// end-to-end distance is at most the sum of the steps. Each step is bounded by the lemma, which
/// holds uniformly over the base distribution, so the intermediate distributions do not need to be
/// known. Capped at one, since no total-variation distance exceeds it.
///
/// This is kept alongside the multiplicative rule because neither dominates by inspection and the
/// analyzer takes whichever is smaller — the minimum of two sound bounds is sound. In practice the
/// multiplicative rule wins on every group measured so far; the union rule is retained because a
/// future perturbation class whose per-factor ranges are not simultaneously attainable would make
/// the multiplicative composition the loose one.
pub fn union_bound(parts: impl IntoIterator<Item = f64>) -> f64 {
    parts.into_iter().fold(0.0, |acc, part| acc + part).min(1.0)
}
