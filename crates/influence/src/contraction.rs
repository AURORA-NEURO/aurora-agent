//! Propagation along the dependency path.
//!
//! Blueprint 43.28 step 3 says "propagate bounds through the decision algebra" and gives no
//! propagation rule. [`crate::ratio::RatioRange::compose`] supplies one that ignores structure
//! entirely: it treats a perturbation deep in the graph exactly like one adjacent to the target.
//! That is sound and, on any region that mixes, badly loose. This module supplies the rule that
//! uses the path.
//!
//! ## The theorem
//!
//! For a stochastic kernel `M`, the Dobrushin ergodic coefficient
//!
//! ```text
//!     δ(M) = max_{u, u'} TV( M(u, ·), M(u', ·) )
//! ```
//!
//! satisfies `TV(pM, qM) ≤ δ(M)·TV(p, q)` for all distributions `p, q`. Along a chain of kernels
//! the coefficients multiply, so a perturbation introduced at step `k` and measured at step `n` is
//! attenuated by `∏_{m>k} δ(M_m)`. Each `δ` lies in `[0, 1]`, so composition can only tighten the
//! bound — never loosen it — which is why the analyzer can take the minimum of this and the
//! structural bound without argument.
//!
//! The local term at step `k` is the lemma of [`crate::ratio`] applied *per row*: perturbing one
//! row of a stochastic kernel and renormalising it moves that row by at most
//! `(√hi − √lo)/(√hi + √lo)`, and `TV(qM, qM') ≤ Σ_u q(u)·TV(M(u,·), M'(u,·)) ≤ max_u (…)` by
//! convexity of the absolute value. The maximum over rows is taken because the analysis does not
//! know which input state the chain will be in.
//!
//! ## The class this handles, stated so it can be refused
//!
//! A region qualifies when, and only when:
//!
//! 1. every factor carries a table;
//! 2. exactly one factor has arity one — the prior — and every other factor has arity two;
//! 3. the arity-two factors form a simple path over the region's variables, covering all of them;
//! 4. the prior sits at one end of that path and the query's single free variable at the other;
//! 5. every transition is row-stochastic in its downstream variable, to `1e-9`.
//!
//! Anything else returns [`crate::UnknownReason::RegionOutsideMethodClass`] naming which clause
//! failed. Approximating a tree or a loopy graph by "the longest path" would produce a number that
//! is not a bound, and an unsound bound is worse than no bound: the certificate would license
//! omitting evidence that mattered.
//!
//! Branching structures are the obvious next class — the coefficient of a tree is the product
//! along the unique path, and the argument goes through unchanged — and they are not implemented.
//! Loopy regions are harder and are not merely unimplemented: the coefficient of a cycle is not
//! the product of its edges, and there is no drop-in generalisation.

use crate::bound::{Approximation, BoundMethod, InfluenceBound, InfluenceMetric};
use crate::error::{InfluenceError, UnknownReason};
use crate::measure::total_variation_of_rows;
use crate::perturbation::Perturbation;
use crate::ratio::RatioRange;
use bioprism_backends::QueryRegion;
use std::collections::BTreeMap;

const METHOD: &str = "chain_contraction";
const HANDLES: &str =
    "a Markov chain: one arity-one prior, arity-two row-stochastic transitions forming a simple path, one free variable at the far end";
const STOCHASTIC_TOLERANCE: f64 = 1e-9;

/// A region recognised as a Markov chain, with its variables in path order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainStructure {
    variables: Vec<String>,
    source_factor: String,
    transitions: Vec<String>,
}

impl ChainStructure {
    /// Variables from the prior's end to the free variable's end.
    pub fn variables(&self) -> &[String] {
        &self.variables
    }

    pub fn source_factor(&self) -> &str {
        &self.source_factor
    }

    /// Transition factor ids; `transitions[k]` joins `variables[k]` to `variables[k + 1]`.
    pub fn transitions(&self) -> &[String] {
        &self.transitions
    }

    pub fn length(&self) -> usize {
        self.transitions.len()
    }

    /// Position of a factor along the chain: `0` for the prior, `k + 1` for `transitions[k]`.
    pub fn position_of(&self, factor_id: &str) -> Option<usize> {
        if factor_id == self.source_factor {
            return Some(0);
        }
        self.transitions
            .iter()
            .position(|id| id == factor_id)
            .map(|index| index + 1)
    }
}

fn outside(detail: impl Into<String>) -> UnknownReason {
    UnknownReason::RegionOutsideMethodClass {
        method: METHOD.to_string(),
        handles: HANDLES.to_string(),
        detail: detail.into(),
    }
}

/// Recognises the chain class, or names the clause that failed.
pub fn detect(region: &QueryRegion) -> Result<ChainStructure, UnknownReason> {
    if !region.has_tables() {
        return Err(outside("at least one factor carries no potential"));
    }
    let free: Vec<&String> = region.free_variables().iter().collect();
    if free.len() != 1 {
        return Err(outside(format!(
            "the query has {} free variables; the chain bound is stated for one",
            free.len()
        )));
    }
    let Some(terminal) = free.first().cloned() else {
        return Err(outside("the chain has no terminal free variable"));
    };

    let mut priors: Vec<&str> = Vec::new();
    let mut edges: Vec<(&str, [&str; 2])> = Vec::new();
    for factor in region.factors() {
        match factor.scope().len() {
            1 => priors.push(factor.id()),
            2 => {
                let [left, right] = factor.scope() else {
                    return Err(outside(format!(
                        "factor {:?} reported a two-variable scope that could not be read",
                        factor.id()
                    )));
                };
                edges.push((factor.id(), [left.as_str(), right.as_str()]));
            }
            other => {
                return Err(outside(format!(
                    "factor {:?} has arity {other}; the chain class admits only arity one and two",
                    factor.id()
                )))
            }
        }
    }
    if priors.len() != 1 {
        return Err(outside(format!(
            "{} arity-one factors; the chain class admits exactly one prior",
            priors.len()
        )));
    }
    if edges.len() + 1 != region.variable_count() {
        return Err(outside(format!(
            "{} transitions over {} variables; a simple path needs exactly one fewer transition than variables",
            edges.len(),
            region.variable_count()
        )));
    }

    let mut adjacency: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for (id, [left, right]) in &edges {
        adjacency.entry(left).or_default().push((right, id));
        adjacency.entry(right).or_default().push((left, id));
    }

    let Some(start) = region
        .factors()
        .iter()
        .find(|factor| factor.id() == priors[0])
        .and_then(|factor| factor.scope().first())
        .cloned()
    else {
        return Err(outside("the prior factor has no source variable"));
    };
    if start.as_str() == terminal.as_str() && !edges.is_empty() {
        return Err(outside(
            "the prior and the free variable are the same variable, but the region has transitions",
        ));
    }
    if adjacency.get(start.as_str()).map_or(0, Vec::len) > 1 {
        return Err(outside(format!(
            "variable {start:?} carries the prior but has degree {} in the transition graph; the prior must sit at an end of the path",
            adjacency[start.as_str()].len()
        )));
    }

    let mut variables = vec![start.clone()];
    let mut transitions: Vec<String> = Vec::new();
    let mut previous: Option<&str> = None;
    let mut current: &str = start.as_str();
    loop {
        let neighbours = adjacency.get(current).cloned().unwrap_or_default();
        if neighbours.len() > 2 {
            return Err(outside(format!(
                "variable {current:?} has degree {}; the transition graph must be a simple path",
                neighbours.len()
            )));
        }
        let next = neighbours
            .into_iter()
            .find(|(other, _)| Some(*other) != previous);
        match next {
            None => break,
            Some((other, factor_id)) => {
                if variables.iter().any(|seen| seen == other) {
                    return Err(outside(
                        "the transition graph contains a cycle; Dobrushin coefficients do not compose around one",
                    ));
                }
                transitions.push(factor_id.to_string());
                variables.push(other.to_string());
                previous = Some(current);
                current = other;
            }
        }
    }

    if variables.len() != region.variable_count() {
        return Err(outside(format!(
            "the path from the prior covers {} of {} variables; the region is disconnected",
            variables.len(),
            region.variable_count()
        )));
    }
    if variables.last() != Some(&terminal) {
        return Err(outside(format!(
            "the path ends at {:?} but the free variable is {terminal:?}",
            variables.last()
        )));
    }

    let chain = ChainStructure {
        variables,
        source_factor: priors[0].to_string(),
        transitions,
    };
    for index in 0..chain.transitions.len() {
        let rows =
            kernel_rows(region, &chain, index).map_err(|error| outside(error.to_string()))?;
        for row in &rows {
            let mass: f64 = row.iter().sum();
            if (mass - 1.0).abs() > STOCHASTIC_TOLERANCE {
                return Err(outside(format!(
                    "transition {:?} has a row summing to {mass}; the contraction argument is stated for stochastic kernels",
                    chain.transitions[index]
                )));
            }
        }
    }
    Ok(chain)
}

/// The rows of transition `index`, indexed by the upstream variable's value.
fn kernel_rows(
    region: &QueryRegion,
    chain: &ChainStructure,
    index: usize,
) -> Result<Vec<Vec<f64>>, InfluenceError> {
    let Some(factor_id) = chain.transitions.get(index) else {
        return Err(InfluenceError::InvalidChain {
            detail: format!("transition index {index} is outside the chain"),
        });
    };
    let factor = region
        .factors()
        .iter()
        .find(|candidate| candidate.id() == factor_id)
        .ok_or_else(|| InfluenceError::UnknownFactor {
            region: region.label().to_string(),
            factor: factor_id.clone(),
        })?;
    let table = factor
        .table()
        .ok_or_else(|| InfluenceError::UntabledFactor {
            factor: factor_id.clone(),
        })?;

    let Some(upstream) = chain.variables.get(index) else {
        return Err(InfluenceError::InvalidChain {
            detail: format!("upstream variable for transition index {index} is missing"),
        });
    };
    let Some(downstream) = index
        .checked_add(1)
        .and_then(|next| chain.variables.get(next))
    else {
        return Err(InfluenceError::InvalidChain {
            detail: format!("downstream variable for transition index {index} is missing"),
        });
    };
    let Some(up_card) = region.cardinality_of(upstream) else {
        return Err(InfluenceError::InvalidChain {
            detail: format!("upstream variable {upstream:?} has no cardinality"),
        });
    };
    let Some(down_card) = region.cardinality_of(downstream) else {
        return Err(InfluenceError::InvalidChain {
            detail: format!("downstream variable {downstream:?} has no cardinality"),
        });
    };

    let Some(first_scope_variable) = factor.scope().first() else {
        return Err(InfluenceError::InvalidChain {
            detail: format!("transition factor {factor_id:?} has no scope"),
        });
    };
    let upstream_first = first_scope_variable == upstream;
    let mut rows = Vec::with_capacity(up_card);
    for u in 0..up_card {
        let mut row = Vec::with_capacity(down_card);
        for d in 0..down_card {
            let offset = if upstream_first {
                u * down_card + d
            } else {
                d * up_card + u
            };
            let Some(value) = table.get(offset).copied() else {
                return Err(InfluenceError::InvalidChain {
                    detail: format!(
                        "transition factor {factor_id:?} is shorter than its declared table shape"
                    ),
                });
            };
            row.push(value);
        }
        rows.push(row);
    }
    Ok(rows)
}

/// `δ(M)` for every transition, in path order.
pub fn dobrushin_coefficients(
    region: &QueryRegion,
    chain: &ChainStructure,
) -> Result<Vec<f64>, InfluenceError> {
    let mut coefficients = Vec::with_capacity(chain.transitions.len());
    for index in 0..chain.transitions.len() {
        let rows = kernel_rows(region, chain, index)?;
        let mut delta = 0.0f64;
        for (i, left) in rows.iter().enumerate() {
            for right in rows.iter().skip(i + 1) {
                delta = delta.max(total_variation_of_rows(left, right));
            }
        }
        coefficients.push(delta.clamp(0.0, 1.0));
    }
    Ok(coefficients)
}

/// The largest per-row total-variation move the perturbation can make to one kernel.
fn local_move(rows: &[Vec<f64>], perturbation: &Perturbation) -> Result<f64, InfluenceError> {
    let mut worst = 0.0f64;
    for row in rows {
        let range = match perturbation {
            Perturbation::Removal => RatioRange::of_removal(row)?,
            Perturbation::MultiplicativeRange { range } => *range,
        };
        worst = worst.max(range.total_variation_bound());
    }
    Ok(worst)
}

/// A bound on the influence of perturbing one factor of a recognised chain.
///
/// The prior is position zero and is attenuated by every transition; transition `k` is position
/// `k + 1` and is attenuated by the transitions after it. A perturbation of the last transition is
/// not attenuated at all, and the bound then coincides with the structural one — correctly, since
/// there is no path left for it to travel down.
pub fn chain_contraction_bound(
    region: &QueryRegion,
    chain: &ChainStructure,
    factor_id: &str,
    perturbation: &Perturbation,
) -> Result<Result<InfluenceBound, UnknownReason>, InfluenceError> {
    let Some(position) = chain.position_of(factor_id) else {
        return Ok(Err(outside(format!(
            "factor {factor_id:?} is not part of the detected chain"
        ))));
    };
    let coefficients = dobrushin_coefficients(region, chain)?;

    let local = if position == 0 {
        let table = crate::perturbed::table_of(region, factor_id)?;
        let range = match perturbation {
            Perturbation::Removal => RatioRange::of_removal(table)?,
            Perturbation::MultiplicativeRange { range } => *range,
        };
        range.total_variation_bound()
    } else {
        let rows = kernel_rows(region, chain, position - 1)?;
        local_move(&rows, perturbation)?
    };

    let attenuation: f64 = coefficients.iter().skip(position).product();
    let value = (local * attenuation).clamp(0.0, 1.0);

    Ok(Ok(InfluenceBound::new(
        value,
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        BoundMethod::ChainContraction,
        Approximation::ConservativeUpperBound,
        format!(
            "{} perturbation of factor {factor_id:?} at chain position {position} of {}, attenuated by {} Dobrushin coefficient(s) with product {attenuation}",
            perturbation.class_name(),
            chain.length(),
            coefficients.len().saturating_sub(position)
        ),
    )?))
}
