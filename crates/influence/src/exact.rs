//! The exact case: run the query twice.
//!
//! Everything else in this crate is an inequality. This module is not: for
//! [`crate::Perturbation::Removal`] the counterfactual world is a single, fully specified region,
//! so the true influence is a computation rather than an estimate. Running it is the only way to
//! know how much slack the structural methods are carrying, and it is the reference the
//! brute-force suite measures looseness against.
//!
//! ## Why this reuses `bioprism-backends` rather than eliminating variables itself
//!
//! A second implementation of variable elimination would be a second thing to keep in parity with
//! the first, and the workspace has already paid for cross-implementation parity once. The
//! elimination engine, the order search, the width budget and the typed refusal all live in
//! `bioprism-backends`; this module supplies two regions and subtracts two answers.
//!
//! ## What "exact" is not
//!
//! Exact means the arithmetic answers the question that was asked, not that the question is the
//! interesting one. The removal counterfactual holds every other factor fixed, so it measures the
//! influence of one factor *given the rest of the region*, which is what an omission manifest
//! needs and is not the same as a Shapley-style attribution over subsets. Nothing here decomposes
//! a joint influence into per-factor shares, and 43.28 does not ask for one.
//!
//! Floating-point addition is not associative, so an elimination order and a direct enumeration
//! can disagree in the last bits. `bioprism-backends` documents the regime where they cannot —
//! integer entries and intermediates below `2^53` — and the fixtures here stay inside it or
//! compare with a tolerance.

use crate::bound::{Approximation, BoundMethod, InfluenceBound, InfluenceMetric};
use crate::error::{InfluenceError, UnknownReason};
use crate::measure::{total_variation, AnswerDistribution};
use crate::perturbed;
use bioprism_backends::{Budget, Declined, QueryBackend, QueryRegion, VariableElimination};

/// A backend refusal, kept as a reason rather than converted into a large number.
fn declined(source: Declined) -> UnknownReason {
    UnknownReason::BackendDeclined {
        detail: source.to_string(),
    }
}

/// The exact total-variation influence of removing one factor, or the reason it could not be run.
///
/// The outer `Result` is for malformed input; the inner one is for a well-formed question the
/// backend declined. Collapsing the two would let a width budget look like a caller bug.
pub fn exact_removal_influence(
    region: &QueryRegion,
    factor_id: &str,
    budget: Budget,
) -> Result<Result<f64, UnknownReason>, InfluenceError> {
    let table = perturbed::table_of(region, factor_id)?;
    if table.is_empty() {
        return Ok(Ok(0.0));
    }
    let removed = perturbed::with_factor_removed(region, factor_id)?;

    let engine = VariableElimination::default().with_budget(budget);
    let base = match engine.execute(region) {
        Ok(computed) => computed,
        Err(source) => return Ok(Err(declined(source))),
    };
    let perturbed_answer = match engine.execute(&removed) {
        Ok(computed) => computed,
        Err(source) => return Ok(Err(declined(source))),
    };

    let base = AnswerDistribution::normalise(&base)?;
    let perturbed_answer = AnswerDistribution::normalise(&perturbed_answer)?;
    Ok(Ok(total_variation(&base, &perturbed_answer)?))
}

/// [`exact_removal_influence`] packaged as a bound.
pub fn exact_removal_bound(
    region: &QueryRegion,
    factor_id: &str,
    budget: Budget,
) -> Result<Result<InfluenceBound, UnknownReason>, InfluenceError> {
    match exact_removal_influence(region, factor_id, budget)? {
        Err(reason) => Ok(Err(reason)),
        Ok(value) => Ok(Ok(InfluenceBound::new(
            value,
            InfluenceMetric::TotalVariationOnNormalisedAnswer,
            BoundMethod::ExactRemoval,
            Approximation::Exact,
            format!(
                "removal of factor {factor_id:?} from region {:?}, all other factors held fixed",
                region.label()
            ),
        )?)),
    }
}

/// The exact influence of removing a whole group of factors at once.
///
/// Not the sum of the individual influences and not bounded by it in either direction: two
/// factors can cancel, so a group whose members each move the answer can be jointly inert, and two
/// factors can reinforce. This is why [`crate::analysis`] composes *bounds* rather than exact
/// values when it has to cover a group without executing.
pub fn exact_group_removal_influence(
    region: &QueryRegion,
    factor_ids: &[String],
    budget: Budget,
) -> Result<Result<f64, UnknownReason>, InfluenceError> {
    let mut stripped = region.clone();
    for id in factor_ids {
        stripped = perturbed::with_factor_removed(&stripped, id)?;
    }

    let engine = VariableElimination::default().with_budget(budget);
    let base = match engine.execute(region) {
        Ok(computed) => computed,
        Err(source) => return Ok(Err(declined(source))),
    };
    let after = match engine.execute(&stripped) {
        Ok(computed) => computed,
        Err(source) => return Ok(Err(declined(source))),
    };

    let base = AnswerDistribution::normalise(&base)?;
    let after = AnswerDistribution::normalise(&after)?;
    Ok(Ok(total_variation(&base, &after)?))
}
