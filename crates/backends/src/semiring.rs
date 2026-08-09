//! The aggregation algebras elimination is allowed to reorder.
//!
//! Blueprint 43.19 writes the compiled query as `Q(x_F) = ⊕_{x_{V∖F}} ⊗_i φ_i(x_{S_i})` and
//! 43.08 makes the choice of `(⊕, ⊗)` an explicit parameter rather than a hard-coded sum and
//! product. The non-negotiable invariant in 43.19 is that "aggregate reordering occurs only when
//! algebra and quantifier semantics permit": variable elimination is sound exactly because ⊗
//! distributes over ⊕ and both are commutative and associative, so a factor that does not mention
//! the eliminated variable can be pulled outside the aggregation.
//!
//! Every variant below is a commutative semiring on the non-negative reals, so all three are legal
//! for reordering. Semirings that are *not* — ordered or non-distributive carriers, and the
//! provenance polynomials of 43.08 — are deliberately absent: adding them here without adding the
//! legality check would let the optimizer reorder an aggregate it must not touch.
//!
//! One caveat these operations cannot enforce. Floating-point addition is not associative, so a
//! reordered sum-product may differ from enumeration in the last bits. It cannot when every entry
//! and every intermediate is an integer below `2^53`, which is the regime the crate's cross-checks
//! against brute force stay inside; a caller with fractional weights must compare with a tolerance
//! rather than with [`crate::ComputedRegion::agrees_exactly_with`]. Nothing in the type system
//! distinguishes the two regimes, and this crate does not pretend otherwise.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Semiring {
    /// `(+, ×)` on the non-negative reals. Marginalisation, model counting, weighted counting.
    SumProduct,
    /// `(max, ×)`. Most-probable-explanation value; the assignment itself is not reconstructed
    /// here, only its weight.
    MaxProduct,
    /// `(max, min)` on `{0, 1}`. Constraint feasibility.
    Boolean,
}

impl Semiring {
    pub fn zero(self) -> f64 {
        0.0
    }

    pub fn one(self) -> f64 {
        1.0
    }

    pub fn add(self, left: f64, right: f64) -> f64 {
        match self {
            Semiring::SumProduct => left + right,
            Semiring::MaxProduct | Semiring::Boolean => {
                if left >= right {
                    left
                } else {
                    right
                }
            }
        }
    }

    pub fn mul(self, left: f64, right: f64) -> f64 {
        match self {
            Semiring::SumProduct | Semiring::MaxProduct => left * right,
            Semiring::Boolean => {
                if left <= right {
                    left
                } else {
                    right
                }
            }
        }
    }

    /// Whether a value is admissible in this algebra's carrier.
    ///
    /// `zero()` is the additive identity only on the non-negative reals: `max(0, -1) = 0 ≠ -1`,
    /// so a negative entry would silently break the accumulation the executor performs. Rejecting
    /// it at region construction is cheaper than discovering it as a wrong answer.
    pub fn admits(self, value: f64) -> bool {
        if !value.is_finite() || value < 0.0 {
            return false;
        }
        match self {
            Semiring::SumProduct | Semiring::MaxProduct => true,
            Semiring::Boolean => value == 0.0 || value == 1.0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Semiring::SumProduct => "sum_product",
            Semiring::MaxProduct => "max_product",
            Semiring::Boolean => "boolean",
        }
    }
}
