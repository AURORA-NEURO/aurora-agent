//! The interval domain over accumulated answer displacement — where the widening is real.
//!
//! A [`Displacement`] abstracts a *total-variation budget*: how far a perturbation can have moved
//! an answer by the time it has travelled some set of paths. It is not a total-variation distance
//! until it is read out, which is the reason its concrete class is `[0, ∞)` rather than `[0, 1]`.
//! A sum over paths can exceed one long before the distance it bounds does, and clamping the
//! accumulator at one *inside* a fixed-point iteration would change the fixed point rather than the
//! answer — the intermediate would be replaced by a smaller number, and a smaller number in the
//! middle of a monotone accumulation is not conservative. [`Displacement::total_variation_bound`]
//! clamps at the end, where clamping is nothing more than the observation that a distance between
//! two probability measures never exceeds one.
//!
//! ## The concretisation
//!
//! ```text
//!     γ(⊥)          =  ∅
//!     γ([lo, hi])   =  { d ∈ ℝ , 0 ≤ d < ∞ : lo ≤ d ≤ hi }
//! ```
//!
//! Structurally the same lattice as [`crate::domains::RatioInterval`], and semantically unrelated
//! to it: one is a reweighting of a measure, the other a distance between two answers. They are
//! separate types so that no transformer can take one for the other, and separate registry entries
//! so that the dynamic path cannot either. [`crate::domain`] argues that at length; this pair is
//! the reason it needed arguing.
//!
//! ## The transformers
//!
//! | `f#` | concrete `f` | soundness |
//! |---|---|---|
//! | [`scale`] | `d ↦ c·d` for `c ≥ 0` | `c·γ(a) ⊆ γ(scale(a, c))` |
//! | [`add`] | `(d, e) ↦ d + e` | `γ(a) + γ(b) ⊆ γ(add(a, b))` |
//! | [`from_ratio`] | the lemma of [`crate::ratio`] | `{TV(p, p') : reweighting in γ(r)} ⊆ γ(from_ratio(r))` |
//!
//! [`from_ratio`] is the one cross-domain transformer this crate ships, and it is deliberately a
//! free function that *takes* a [`crate::domains::RatioInterval`] and *returns* a `Displacement`
//! rather than a method on either domain. A conversion that looks like a lattice operation invites
//! being used as one.
//!
//! ## Why widening here needs a threshold ladder
//!
//! The classical `0 / ∞` widening of [`crate::domains::ratio_interval`] terminates the chain and
//! lands on `⊤ = [0, ∞]`, and in this domain that is a dead end: the accumulation
//! `u ↦ b + C·u` maps `∞` to `∞`, so the descending iteration that is supposed to recover the
//! precision widening gave away has nowhere to descend from. The result would be a permanent
//! bound of one on every loopy region — sound, and never worth computing.
//!
//! [`WIDENING_THRESHOLDS`] is the standard repair: an unstable upper endpoint jumps to the smallest
//! power of two above the proposed value rather than to infinity. Termination is unaffected — the
//! ladder is finite and the endpoint only ever climbs it — and the landing point is finite whenever
//! the true fixed point is, so narrowing has somewhere to go. `∞` remains the last rung, for the
//! chains that genuinely diverge.

use crate::domain::{AbstractDomain, DomainId, EnumerableConcretisation, FactClass};
use crate::domains::ratio_interval::RatioInterval;
use crate::error::InfluenceError;
use serde::{Deserialize, Serialize};

/// The registered name of [`DisplacementDomain`].
pub const DISPLACEMENT_DOMAIN: &str = "answer_displacement_interval";

/// The rungs an unstable upper endpoint may land on, in ascending order, ending at `∞`.
///
/// Powers of two from one to `2^30`. The ladder is fixed rather than derived from the analysis so
/// that the same region always widens to the same element: a widening that depended on the values
/// it had seen would make the reported bound depend on iteration order.
pub const WIDENING_THRESHOLDS: usize = 31;

fn threshold_at_or_above(value: f64) -> f64 {
    if !value.is_finite() {
        return f64::INFINITY;
    }
    let mut rung = 1.0f64;
    for _ in 0..WIDENING_THRESHOLDS {
        if rung >= value {
            return rung;
        }
        rung *= 2.0;
    }
    f64::INFINITY
}

/// An interval of accumulated total-variation budget, or `⊥`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "element")]
pub enum Displacement {
    /// `γ = ∅`. Distinct from `[0, 0]`, which says the answer provably does not move.
    Bottom,
    /// `0 ≤ lo ≤ hi ≤ ∞`.
    Range { lo: f64, hi: f64 },
}

impl Displacement {
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
        Ok(Displacement::Range { lo, hi })
    }

    /// The exact displacement `value`, known to be neither more nor less.
    pub fn exactly(value: f64) -> Result<Self, InfluenceError> {
        Displacement::range(value, value)
    }

    /// Displacement known only to be at most `value`.
    pub fn at_most(value: f64) -> Result<Self, InfluenceError> {
        Displacement::range(0.0, value)
    }

    pub fn endpoints(self) -> Option<(f64, f64)> {
        match self {
            Displacement::Bottom => None,
            Displacement::Range { lo, hi } => Some((lo, hi)),
        }
    }

    /// The bound a certificate would carry: the upper endpoint, capped at one.
    ///
    /// Capping here rather than during accumulation is the whole reason the concrete class is
    /// unbounded; the module header says why. `⊥` reports `0.0` for the reason
    /// [`crate::domains::RatioInterval::total_variation_bound`] gives.
    pub fn total_variation_bound(self) -> f64 {
        match self {
            Displacement::Bottom => 0.0,
            Displacement::Range { hi, .. } => {
                if hi.is_nan() {
                    1.0
                } else {
                    hi.clamp(0.0, 1.0)
                }
            }
        }
    }

    /// Whether the upper endpoint is finite, i.e. whether anything was learned at all.
    pub fn is_bounded_above(self) -> bool {
        match self {
            Displacement::Bottom => true,
            Displacement::Range { hi, .. } => hi.is_finite(),
        }
    }
}

/// `c·γ(a) ⊆ γ(scale(a, c))` for `c ≥ 0`.
///
/// Used with a Dobrushin coefficient, where `c ∈ [0, 1]` and the operation is attenuation along one
/// dependency step. A negative or non-finite `c` has no reading as an attenuation and saturates the
/// result rather than producing a NaN endpoint.
pub fn scale(element: Displacement, coefficient: f64) -> Displacement {
    let Some((lo, hi)) = element.endpoints() else {
        return Displacement::Bottom;
    };
    if !coefficient.is_finite() || coefficient < 0.0 {
        return Displacement::Range {
            lo: 0.0,
            hi: f64::INFINITY,
        };
    }
    Displacement::Range {
        lo: lo * coefficient,
        hi: if coefficient == 0.0 || hi == 0.0 {
            0.0
        } else {
            hi * coefficient
        },
    }
}

/// `γ(a) + γ(b) ⊆ γ(add(a, b))`.
///
/// The triangle inequality for total variation, lifted. `⊥` is absorbing: adding an unreachable
/// contribution to a reachable one leaves the sum unreachable, which is the standard reading and
/// the one the solver relies on when it starts an accumulation from `⊥`.
pub fn add(left: Displacement, right: Displacement) -> Displacement {
    let (Some((llo, lhi)), Some((rlo, rhi))) = (left.endpoints(), right.endpoints()) else {
        return Displacement::Bottom;
    };
    Displacement::Range {
        lo: llo + rlo,
        hi: lhi + rhi,
    }
}

/// The lemma of [`crate::ratio`] as a transformer between two registered domains.
///
/// Every reweighting in `γ(r)` moves a normalised answer by at most
/// `(√hi − √lo)/(√hi + √lo)`, and by at least nothing, so the image lands in `[0, that]`. The
/// lower endpoint is zero rather than anything tighter because the lemma is an upper bound only:
/// a reweighting inside a wide interval is permitted to be the identity.
pub fn from_ratio(reweighting: RatioInterval) -> Displacement {
    match reweighting {
        RatioInterval::Bottom => Displacement::Bottom,
        other => Displacement::Range {
            lo: 0.0,
            hi: other.total_variation_bound(),
        },
    }
}

/// The interval lattice over accumulated answer displacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DisplacementDomain;

impl AbstractDomain for DisplacementDomain {
    type Element = Displacement;
    type Concrete = f64;

    fn id(&self) -> DomainId {
        DomainId::new(DISPLACEMENT_DOMAIN)
    }

    fn abstracts(&self) -> FactClass {
        FactClass::AnswerDisplacement
    }

    fn bottom(&self) -> Displacement {
        Displacement::Bottom
    }

    fn top(&self) -> Displacement {
        Displacement::Range {
            lo: 0.0,
            hi: f64::INFINITY,
        }
    }

    fn leq(&self, left: &Displacement, right: &Displacement) -> bool {
        match (left.endpoints(), right.endpoints()) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some((llo, lhi)), Some((rlo, rhi))) => rlo <= llo && lhi <= rhi,
        }
    }

    fn join(&self, left: &Displacement, right: &Displacement) -> Displacement {
        match (left.endpoints(), right.endpoints()) {
            (None, _) => *right,
            (_, None) => *left,
            (Some((llo, lhi)), Some((rlo, rhi))) => Displacement::Range {
                lo: llo.min(rlo),
                hi: lhi.max(rhi),
            },
        }
    }

    fn meet(&self, left: &Displacement, right: &Displacement) -> Displacement {
        match (left.endpoints(), right.endpoints()) {
            (None, _) | (_, None) => Displacement::Bottom,
            (Some((llo, lhi)), Some((rlo, rhi))) => {
                let lo = llo.max(rlo);
                let hi = lhi.min(rhi);
                if lo > hi {
                    Displacement::Bottom
                } else {
                    Displacement::Range { lo, hi }
                }
            }
        }
    }

    /// Widening with thresholds: an unstable upper endpoint climbs to the next power of two.
    ///
    /// Terminates because the endpoint is non-decreasing across a widening sequence and, once it
    /// has moved at all, only ever takes values from a ladder of [`WIDENING_THRESHOLDS`] rungs plus
    /// `∞`. The lower endpoint drops to zero at most once. So no widening sequence exceeds
    /// `WIDENING_THRESHOLDS + 2` strict increases regardless of the transformer.
    fn widen(&self, previous: &Displacement, next: &Displacement) -> Displacement {
        match (previous.endpoints(), next.endpoints()) {
            (None, _) => *next,
            (_, None) => *previous,
            (Some((plo, phi)), Some((nlo, nhi))) => Displacement::Range {
                lo: if nlo < plo { 0.0 } else { plo },
                hi: if nhi > phi {
                    threshold_at_or_above(nhi)
                } else {
                    phi
                },
            },
        }
    }

    fn concretises(&self, element: &Displacement, concrete: &f64) -> bool {
        match element.endpoints() {
            None => false,
            Some((lo, hi)) => concrete.is_finite() && *concrete >= lo && *concrete <= hi,
        }
    }

    fn render(&self, element: &Displacement) -> String {
        match element.endpoints() {
            None => "⊥".to_string(),
            Some((lo, hi)) => format!("[{lo}, {hi}]"),
        }
    }
}

impl EnumerableConcretisation for DisplacementDomain {
    /// A grid through `[0, 4]` at sixteenths, plus the unit endpoints a distance actually reaches.
    ///
    /// A grid, not the class: the same falsification-search caveat
    /// [`crate::domains::RatioIntervalDomain`] carries.
    fn concrete_universe(&self) -> Vec<f64> {
        (0..=64).map(|step| f64::from(step) / 16.0).collect()
    }

    fn universe_is_complete(&self) -> bool {
        false
    }
}
