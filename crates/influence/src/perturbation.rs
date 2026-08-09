//! What "this evidence might have been different" is allowed to mean.
//!
//! An influence bound is meaningless without a perturbation class: "how much can the answer move"
//! is only a question once you say what is permitted to move. Blueprint 43.28 requires a bound to
//! carry its "validity scope" and never enumerates the classes, so the two below are decisions of
//! this implementation.
//!
//! [`Perturbation::Removal`] is the one the omission manifest actually needs. An omitted fact is a
//! fact the compiler did not put in the context, and the honest counterfactual is the region
//! evaluated as if that evidence had never been acquired — the factor replaced by the all-ones
//! potential, which under a normalised answer is indistinguishable from replacing it by any
//! constant. This class is *exactly* computable, which is why [`crate::exact`] exists.
//!
//! [`Perturbation::MultiplicativeRange`] is the one a stated measurement range needs. An assay
//! whose value is known to within a factor of two induces a reweighting of the joint measure
//! inside `[1/2, 2]`, and the lemma of [`crate::ratio`] converts that directly into a bound with
//! no execution at all. It handles the case removal cannot: a fact that *is* present but whose
//! value is uncertain.
//!
//! Classes deliberately absent, because no sound method here covers them:
//!
//! - **Additive perturbation.** `φ ± δ` does not induce a bounded multiplicative reweighting when
//!   an entry can approach zero, so the lemma does not apply and the bound would have to be
//!   derived some other way.
//! - **Structural perturbation.** Adding a factor the world does not declare changes the region's
//!   scope, and every method here assumes the scope is fixed. This is the case the reference
//!   world's own limitation about an incomplete factor graph refers to, and it is not addressed.

use crate::error::InfluenceError;
use crate::ratio::RatioRange;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "class")]
pub enum Perturbation {
    /// The factor is replaced by the all-ones potential: the evidence was never acquired.
    Removal,
    /// Every entry may be independently rescaled by any value in the range.
    MultiplicativeRange { range: RatioRange },
}

impl Perturbation {
    /// A range known to a relative tolerance, e.g. `0.1` for ±10%.
    ///
    /// Rejects tolerances at or above one, where the lower endpoint would reach zero and the
    /// resulting bound would be the vacuous `1.0`. A caller who genuinely means "this value could
    /// be anything" should say [`crate::UnknownReason`], not encode it as a range.
    pub fn relative_tolerance(tolerance: f64) -> Result<Self, InfluenceError> {
        if !tolerance.is_finite() || !(0.0..1.0).contains(&tolerance) {
            return Err(InfluenceError::InadmissibleRatioEndpoint { value: tolerance });
        }
        Ok(Perturbation::MultiplicativeRange {
            range: RatioRange::new(1.0 - tolerance, 1.0 + tolerance)?,
        })
    }

    pub fn class_name(&self) -> &'static str {
        match self {
            Perturbation::Removal => "removal",
            Perturbation::MultiplicativeRange { .. } => "multiplicative_range",
        }
    }

    /// The interval the joint measure's pointwise reweighting is confined to.
    ///
    /// For [`Perturbation::Removal`] this is derived from the factor's own entries, so a factor
    /// with no table cannot be classed here; the caller must have already rejected that case.
    /// For [`Perturbation::MultiplicativeRange`] the range is stated and the table is irrelevant,
    /// which is what lets a stated-range bound be computed on a region the executor would decline.
    pub fn ratio_range(&self, table: &[f64]) -> Result<RatioRange, InfluenceError> {
        match self {
            Perturbation::Removal => RatioRange::of_removal(table),
            Perturbation::MultiplicativeRange { range } => Ok(*range),
        }
    }

    /// The table that replaces the original under this perturbation's most extreme admissible
    /// realisation, for a brute-force search to start from.
    ///
    /// Only [`Perturbation::Removal`] has a single realisation; the range class has a continuum,
    /// and [`crate::bruteforce`] enumerates its vertices rather than asking here.
    pub fn single_realisation(&self, table: &[f64]) -> Option<Vec<f64>> {
        match self {
            Perturbation::Removal => Some(vec![1.0; table.len()]),
            Perturbation::MultiplicativeRange { .. } => None,
        }
    }
}
