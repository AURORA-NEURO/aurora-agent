//! The functional influence is measured on.
//!
//! Blueprint 43.28 writes the certified-decision condition as `D_q(z) ⊕ U_omit` lying inside one
//! permitted region, and never says what `D_q(z)` is for a factorised query. This crate takes it
//! to be the *normalised* answer over the query's free variables — the table
//! [`bioprism_backends::ComputedRegion`] returns, divided by its own total.
//!
//! Normalising is not cosmetic. The unnormalised answer scales with every factor in the region, so
//! removing a factor whose entries are all `0.5` would register as a large change in an answer
//! nothing about the decision depends on. Under normalisation that perturbation registers as
//! exactly zero, which is the correct reading: a factor that is constant across its scope carries
//! no information about anything. It also makes the measure comparable across queries and bounded
//! in `[0, 1]`, which is what lets a bound compose along a path without accumulating units.
//!
//! The cost of normalising is that a region whose answer is entirely zero has no normalised form,
//! and that is reported as [`crate::InfluenceError::DegenerateAnswer`] rather than smoothed. A
//! query whose every assignment is impossible is a modelling failure, not a distribution.

use crate::error::InfluenceError;
use bioprism_backends::ComputedRegion;

/// A normalised answer over the query's free variables.
#[derive(Debug, Clone, PartialEq)]
pub struct AnswerDistribution {
    scope: Vec<String>,
    mass: Vec<f64>,
}

impl AnswerDistribution {
    /// Divides a computed answer by its own total.
    ///
    /// The total is a plain sum rather than the region's semiring aggregate. Under `max-product`
    /// the semiring aggregate is a maximum, which does not normalise a table into a distribution;
    /// treating a max-product answer as a distribution and taking total variation between two of
    /// them would be a category error dressed as a measurement. Callers wanting influence on a
    /// most-probable-explanation value need a different functional and this crate does not
    /// provide one.
    pub fn normalise(computed: &ComputedRegion) -> Result<Self, InfluenceError> {
        Self::from_parts(computed.scope().to_vec(), computed.values().to_vec())
    }

    pub fn from_parts(scope: Vec<String>, values: Vec<f64>) -> Result<Self, InfluenceError> {
        let total: f64 = values.iter().sum();
        if !total.is_finite() || total <= 0.0 {
            return Err(InfluenceError::DegenerateAnswer { mass: total });
        }
        Ok(AnswerDistribution {
            scope,
            mass: values.into_iter().map(|value| value / total).collect(),
        })
    }

    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    pub fn mass(&self) -> &[f64] {
        &self.mass
    }
}

/// Half the L1 distance between two answers over the same scope.
///
/// Differing scopes are a structural disagreement, not a numerical one, and are rejected rather
/// than reported as a large distance — a caller comparing answers over different free variables
/// has a bug that a number would hide.
pub fn total_variation(
    left: &AnswerDistribution,
    right: &AnswerDistribution,
) -> Result<f64, InfluenceError> {
    if left.scope != right.scope || left.mass.len() != right.mass.len() {
        return Err(InfluenceError::IncomparableScopes {
            left: left.scope.clone(),
            right: right.scope.clone(),
        });
    }
    let sum: f64 = left
        .mass
        .iter()
        .zip(&right.mass)
        .map(|(a, b)| (a - b).abs())
        .sum();
    Ok((0.5 * sum).clamp(0.0, 1.0))
}

/// Half the L1 distance between two rows of a conditional table, without normalising.
///
/// Used by [`crate::contraction`], where the rows are already stochastic by hypothesis and
/// renormalising would mask a violation of that hypothesis rather than surface it.
pub(crate) fn total_variation_of_rows(left: &[f64], right: &[f64]) -> f64 {
    let sum: f64 = left
        .iter()
        .zip(right)
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>();
    (0.5 * sum).clamp(0.0, 1.0)
}
