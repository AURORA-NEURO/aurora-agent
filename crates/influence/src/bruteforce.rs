//! Ground truth by exhaustion.
//!
//! This module is the crate's reason to exist. Every bound above it is an argument; this is the
//! only thing that can catch the argument being wrong. It takes a region, a set of factors and a
//! perturbation class, walks the perturbation space, and reports the largest total-variation move
//! it found together with the perturbed tables that achieved it.
//!
//! ## What exhaustion means for each class, stated precisely
//!
//! [`crate::Perturbation::Removal`] has exactly one realisation — the all-ones potential — so the
//! number returned is the *true* maximum influence, not an estimate of it. A soundness assertion
//! against it is a proof for that region.
//!
//! [`crate::Perturbation::MultiplicativeRange`] has a continuum. This module enumerates every
//! vertex of the box (each entry independently at the low or the high endpoint) and additionally
//! evaluates a seeded grid of interior points. That is a *falsification search*: it returns a
//! lower bound on the true maximum, so `found ≤ reported_bound` passing is evidence of soundness
//! and `found > reported_bound` is a proof of unsoundness. This crate does not claim the maximum
//! over the box is always attained at a vertex, and does not rely on it — the asymmetry is stated
//! rather than assumed away, because assuming it would turn a falsification test into a
//! verification claim it has not earned.
//!
//! ## Why the executor here is not the one the bound used
//!
//! The exact bound in [`crate::exact`] runs bucket elimination. Ground truth here runs
//! [`bioprism_backends::DirectMaterialization`], which enumerates the joint assignment space and
//! never reorders an aggregation. Using the same engine for both would make the soundness suite
//! insensitive to a bug in that engine, which is the class of bug most likely to make a bound
//! look sound while being wrong.
//!
//! ## The cap
//!
//! [`MAX_PERTURBATION_VERTICES`] refuses rather than sampling. A soundness check that silently
//! degrades to a sample when the space gets large is a check that gets weaker exactly where the
//! bounds get interesting.

use crate::error::InfluenceError;
use crate::measure::{total_variation, AnswerDistribution};
use crate::perturbation::Perturbation;
use crate::perturbed;
use crate::rng::SplitMix64;
use bioprism_backends::{Budget, DirectMaterialization, QueryBackend, QueryRegion};

/// The largest perturbation space this module will enumerate.
pub const MAX_PERTURBATION_VERTICES: u64 = 4096;

/// The outcome of an exhaustive search.
#[derive(Debug, Clone, PartialEq)]
pub struct BruteForceResult {
    /// The largest total-variation move found. Exact for removal, a lower bound otherwise.
    pub found_influence: f64,
    /// How many perturbed regions were actually executed.
    pub evaluated: usize,
    /// Whether the search covered the whole space or searched a subset of a continuum.
    pub exhaustive: bool,
    /// The tables that achieved `found_influence`, aligned with the requested factor ids.
    pub witness: Vec<Vec<f64>>,
}

fn declined(source: bioprism_backends::Declined) -> InfluenceError {
    InfluenceError::BruteForceDeclined {
        detail: source.to_string(),
    }
}

fn answer(region: &QueryRegion) -> Result<AnswerDistribution, InfluenceError> {
    let computed = DirectMaterialization::new()
        .with_budget(Budget::default())
        .execute(region)
        .map_err(declined)?;
    AnswerDistribution::normalise(&computed)
}

/// The maximum influence a perturbation of `factor_ids` can have on the region's answer.
///
/// `interior_samples` is the number of seeded points drawn from inside the box in addition to its
/// vertices; it is ignored for [`crate::Perturbation::Removal`], which has no interior.
pub fn maximum_influence(
    region: &QueryRegion,
    factor_ids: &[String],
    perturbation: &Perturbation,
    interior_samples: usize,
    seed: u64,
) -> Result<BruteForceResult, InfluenceError> {
    let mut originals: Vec<Vec<f64>> = Vec::with_capacity(factor_ids.len());
    for id in factor_ids {
        originals.push(perturbed::table_of(region, id)?.to_vec());
    }
    let baseline = answer(region)?;

    let mut best = 0.0f64;
    let mut best_witness: Vec<Vec<f64>> = originals.clone();
    let mut evaluated = 0usize;

    let evaluate = |candidate: &[Vec<f64>],
                    best: &mut f64,
                    best_witness: &mut Vec<Vec<f64>>,
                    evaluated: &mut usize|
     -> Result<(), InfluenceError> {
        let mut altered = region.clone();
        for (id, table) in factor_ids.iter().zip(candidate) {
            altered = perturbed::with_replaced_table(
                &altered,
                id,
                table.clone(),
                "brute-force perturbation of a soundness fixture",
            )?;
        }
        let moved = total_variation(&baseline, &answer(&altered)?)?;
        *evaluated += 1;
        if moved > *best {
            *best = moved;
            *best_witness = candidate.to_vec();
        }
        Ok(())
    };

    let exhaustive = match perturbation {
        Perturbation::Removal => {
            let candidate: Vec<Vec<f64>> = originals
                .iter()
                .map(|table| {
                    perturbation.single_realisation(table).ok_or_else(|| {
                        InfluenceError::BruteForceDeclined {
                            detail: "the selected perturbation has no single realisation".into(),
                        }
                    })
                })
                .collect::<Result<_, _>>()?;
            evaluate(&candidate, &mut best, &mut best_witness, &mut evaluated)?;
            true
        }
        Perturbation::MultiplicativeRange { range } => {
            let total_entries: usize = originals.iter().map(Vec::len).sum();
            if total_entries >= 63 {
                return Err(InfluenceError::BruteForceTooLarge {
                    entries: u64::MAX,
                    cap: MAX_PERTURBATION_VERTICES,
                });
            }
            let vertices = 1u64 << total_entries;
            if vertices > MAX_PERTURBATION_VERTICES {
                return Err(InfluenceError::BruteForceTooLarge {
                    entries: vertices,
                    cap: MAX_PERTURBATION_VERTICES,
                });
            }
            for mask in 0..vertices {
                let mut bit = 0u32;
                let candidate: Vec<Vec<f64>> = originals
                    .iter()
                    .map(|table| {
                        table
                            .iter()
                            .map(|value| {
                                let high = (mask >> bit) & 1 == 1;
                                bit += 1;
                                value * if high { range.hi() } else { range.lo() }
                            })
                            .collect()
                    })
                    .collect();
                evaluate(&candidate, &mut best, &mut best_witness, &mut evaluated)?;
            }
            let mut rng = SplitMix64::new(seed);
            for _ in 0..interior_samples {
                let candidate: Vec<Vec<f64>> = originals
                    .iter()
                    .map(|table| {
                        table
                            .iter()
                            .map(|value| value * rng.between(range.lo(), range.hi()))
                            .collect()
                    })
                    .collect();
                evaluate(&candidate, &mut best, &mut best_witness, &mut evaluated)?;
            }
            false
        }
    };

    Ok(BruteForceResult {
        found_influence: best,
        evaluated,
        exhaustive,
        witness: best_witness,
    })
}
