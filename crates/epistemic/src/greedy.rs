//! Greedy and lazy-greedy evidence selection: blueprint 43.14, steps 3 and 4.
//!
//! ## Protected closure is not a candidate
//!
//! 43.14's first non-negotiable invariant is that "greedy selection never bypasses protected
//! closure", and 43.13 makes the closure mandatory *before* any relevance step. So the protected
//! set is not seeded into the candidate pool with a large bonus — it is unioned in before the
//! first marginal is computed, and its cost is charged against the budget first. A closure that
//! does not fit is [`EpistemicError::ProtectedClosureExceedsBudget`], never a smaller closure.
//! `bioprism-examples` calls this `refusal_over_silent_truncation` and it is the same rule.
//!
//! ## Greedy declines a non-positive step
//!
//! Under a cardinality constraint of `k`, this greedy may return fewer than `k` elements: it stops
//! when no feasible element has a positive marginal. For a monotone objective that never happens
//! before the constraint binds. For a non-monotone one it happens routinely, and the alternative —
//! taking the least-bad negative step to fill the quota — would make the selection worse in
//! exchange for looking complete. The measured ratios in [`crate::optimal`] are of this greedy.
//!
//! ## Lazy greedy is a speed claim, not a quality claim
//!
//! Under submodularity the lazy variant returns the *identical* set, because a stale marginal is
//! an upper bound on the current one and the tie-break is the same. That equality is the only
//! thing asserted about it. This crate makes no claim about how many evaluations it saves — the
//! objectives here are tabulated, so an evaluation costs a table lookup and a timing comparison
//! would measure nothing. Where lazy and plain greedy *disagree*, the objective was not
//! submodular, and [`crate::submodularity`] will say so independently.

use crate::error::EpistemicError;
use crate::objective::SetFunction;
use crate::submodularity::SubmodularityReport;
use crate::theorem::{self, Applicability};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Absolute floor a marginal must clear to be taken.
pub const MARGINAL_EPSILON: f64 = 1e-12;

/// A cardinality bound, a knapsack budget, or both.
///
/// 43.14 also names partition matroids and mixed constraints. Neither is implemented; a caller
/// passing one would have to encode it, and encoding it wrongly is worse than not having it. See
/// [`crate::NOT_IMPLEMENTED`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub cardinality: Option<usize>,
    pub budget: Option<f64>,
    /// Per-element cost, one entry per ground element.
    pub costs: Vec<f64>,
}

impl Constraint {
    /// Builds a selection constraint with any combination of the supported bounds.
    ///
    /// The public convenience constructors cover the common cardinality-only and knapsack-only
    /// cases.  The composed form is useful at transport boundaries where a caller may want both a
    /// maximum number of items and a scalarized cost budget; it still runs through the same
    /// validation, so a missing bound, negative budget, non-finite cost, or zero-cost knapsack
    /// element cannot enter the selector through a deserialization shortcut.
    pub fn bounded(
        cardinality: Option<usize>,
        budget: Option<f64>,
        costs: Vec<f64>,
    ) -> Result<Self, EpistemicError> {
        Constraint {
            cardinality,
            budget,
            costs,
        }
        .validated()
    }

    /// A pure cardinality constraint. Costs are unit, so `rate` reads as a count.
    pub fn cardinality(k: usize, ground: usize) -> Result<Self, EpistemicError> {
        Constraint::bounded(Some(k), None, vec![1.0; ground])
    }

    /// A knapsack constraint with explicit costs.
    pub fn knapsack(budget: f64, costs: Vec<f64>) -> Result<Self, EpistemicError> {
        Constraint::bounded(None, Some(budget), costs)
    }

    fn validated(self) -> Result<Self, EpistemicError> {
        if self.cardinality.is_none() && self.budget.is_none() {
            return Err(EpistemicError::UnconstrainedSelection);
        }
        for (index, cost) in self.costs.iter().enumerate() {
            if !cost.is_finite() || *cost < 0.0 {
                return Err(EpistemicError::InadmissibleCost {
                    item: format!("element {index}"),
                    value: *cost,
                });
            }
            if self.budget.is_some() && *cost <= 0.0 {
                return Err(EpistemicError::InadmissibleCost {
                    item: format!("element {index}"),
                    value: *cost,
                });
            }
        }
        if let Some(budget) = self.budget {
            if !budget.is_finite() || budget < 0.0 {
                return Err(EpistemicError::InadmissibleCost {
                    item: "budget".to_string(),
                    value: budget,
                });
            }
        }
        Ok(self)
    }

    pub fn is_cardinality_only(&self) -> bool {
        self.cardinality.is_some() && self.budget.is_none()
    }

    pub fn cost_of(&self, subset: &BTreeSet<usize>) -> f64 {
        subset.iter().map(|&i| self.costs[i]).sum()
    }

    /// Whether adding `element` to `chosen` stays inside every bound.
    pub fn admits(&self, chosen: &BTreeSet<usize>, element: usize) -> bool {
        if let Some(k) = self.cardinality {
            if chosen.len() + 1 > k {
                return false;
            }
        }
        if let Some(budget) = self.budget {
            if self.cost_of(chosen) + self.costs[element] > budget + MARGINAL_EPSILON {
                return false;
            }
        }
        true
    }

    fn check_ground(&self, ground: usize) -> Result<(), EpistemicError> {
        if self.costs.len() != ground {
            return Err(EpistemicError::ElementOutOfRange {
                element: self.costs.len(),
                ground,
            });
        }
        Ok(())
    }
}

/// One accepted greedy step, in acceptance order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GreedyStep {
    pub element: usize,
    /// Marginal gain at the moment of acceptance.
    pub marginal: f64,
    pub cost: f64,
    /// Candidates whose marginal had to be recomputed before this one was accepted. Zero for
    /// plain greedy, which recomputes everything every step.
    pub reevaluations: usize,
}

/// What a selector chose, what it is worth, and what may be claimed about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    /// The mandatory closure, forced in before any relevance step.
    pub protected: Vec<usize>,
    /// Everything selected, protected included, in index order.
    pub chosen: Vec<usize>,
    pub value: f64,
    pub cost: f64,
    pub steps: Vec<GreedyStep>,
    /// Whether the `1 − 1/e` factor may be quoted, and why not when it may not.
    pub guarantee: Applicability,
    /// Total marginal evaluations. Reported so lazy and plain greedy can be compared on work
    /// done, without either being claimed to be faster in wall-clock terms.
    pub evaluations: usize,
}

impl Selection {
    pub fn as_set(&self) -> BTreeSet<usize> {
        self.chosen.iter().copied().collect()
    }
}

fn start(
    function: &(impl SetFunction + ?Sized),
    constraint: &Constraint,
    protected: &BTreeSet<usize>,
) -> Result<BTreeSet<usize>, EpistemicError> {
    constraint.check_ground(function.ground_size())?;
    for &element in protected {
        if element >= function.ground_size() {
            return Err(EpistemicError::ElementOutOfRange {
                element,
                ground: function.ground_size(),
            });
        }
    }
    if let Some(k) = constraint.cardinality {
        if protected.len() > k {
            return Err(EpistemicError::ProtectedClosureExceedsCardinality {
                cardinality: k,
                protected: protected.len(),
            });
        }
    }
    if let Some(budget) = constraint.budget {
        let cost = constraint.cost_of(protected);
        if cost > budget + MARGINAL_EPSILON {
            return Err(EpistemicError::ProtectedClosureExceedsBudget {
                protected: cost,
                budget,
            });
        }
    }
    Ok(protected.clone())
}

fn score(marginal: f64, cost: f64, knapsack: bool) -> f64 {
    if knapsack {
        marginal / cost
    } else {
        marginal
    }
}

/// Plain greedy: recompute every candidate's marginal at every step.
///
/// `report` gates the guarantee. Pass the output of [`crate::submodularity::check`] to earn a
/// factor; pass `None` to get a selection with [`Applicability::NotChecked`] attached.
pub fn greedy<F: SetFunction + ?Sized>(
    function: &F,
    constraint: &Constraint,
    protected: &BTreeSet<usize>,
    report: Option<&SubmodularityReport>,
) -> Result<Selection, EpistemicError> {
    let mut chosen = start(function, constraint, protected)?;
    let knapsack = constraint.budget.is_some();
    let mut steps = Vec::new();
    let mut evaluations = 0usize;

    loop {
        let mut best: Option<(f64, f64, usize)> = None;
        for element in 0..function.ground_size() {
            if chosen.contains(&element) || !constraint.admits(&chosen, element) {
                continue;
            }
            let marginal = function.marginal(&chosen, element)?;
            evaluations += 1;
            if marginal <= MARGINAL_EPSILON {
                continue;
            }
            let ranked = score(marginal, constraint.costs[element], knapsack);
            if best.is_none_or(|(current, _, _)| ranked > current + MARGINAL_EPSILON) {
                best = Some((ranked, marginal, element));
            }
        }
        let Some((_, marginal, element)) = best else {
            break;
        };
        chosen.insert(element);
        steps.push(GreedyStep {
            element,
            marginal,
            cost: constraint.costs[element],
            reevaluations: 0,
        });
    }

    finish(function, constraint, protected, chosen, steps, evaluations, report)
}

/// Lazy greedy: keep stale marginals as upper bounds and re-evaluate only the current front-runner.
///
/// Correct under submodularity, where a marginal can only fall as the selection grows. On a
/// non-submodular objective a stale bound can *under*state the current marginal, so this may
/// return a different set from [`greedy`] — which is a detection of non-submodularity, and the
/// suite uses it as one.
pub fn lazy_greedy<F: SetFunction + ?Sized>(
    function: &F,
    constraint: &Constraint,
    protected: &BTreeSet<usize>,
    report: Option<&SubmodularityReport>,
) -> Result<Selection, EpistemicError> {
    let mut chosen = start(function, constraint, protected)?;
    let knapsack = constraint.budget.is_some();
    let mut steps = Vec::new();
    let mut evaluations = 0usize;

    let mut queue: Vec<(f64, f64, usize, usize)> = Vec::new();
    for element in 0..function.ground_size() {
        if chosen.contains(&element) {
            continue;
        }
        let marginal = function.marginal(&chosen, element)?;
        evaluations += 1;
        queue.push((
            score(marginal, constraint.costs[element], knapsack),
            marginal,
            element,
            0,
        ));
    }

    let mut round = 0usize;
    loop {
        round += 1;
        queue.retain(|(_, _, element, _)| !chosen.contains(element));
        let mut reevaluations = 0usize;
        let accepted = loop {
            queue.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.2.cmp(&b.2)));
            let Some(&(_, marginal, element, stamp)) = queue.first() else {
                break None;
            };
            if !constraint.admits(&chosen, element) {
                queue.remove(0);
                continue;
            }
            if stamp == round {
                if marginal <= MARGINAL_EPSILON {
                    break None;
                }
                queue.remove(0);
                break Some((marginal, element));
            }
            let fresh = function.marginal(&chosen, element)?;
            evaluations += 1;
            reevaluations += 1;
            queue[0] = (
                score(fresh, constraint.costs[element], knapsack),
                fresh,
                element,
                round,
            );
        };
        let Some((marginal, element)) = accepted else {
            break;
        };
        chosen.insert(element);
        steps.push(GreedyStep {
            element,
            marginal,
            cost: constraint.costs[element],
            reevaluations,
        });
    }

    finish(function, constraint, protected, chosen, steps, evaluations, report)
}

#[allow(clippy::too_many_arguments)]
fn finish<F: SetFunction + ?Sized>(
    function: &F,
    constraint: &Constraint,
    protected: &BTreeSet<usize>,
    chosen: BTreeSet<usize>,
    steps: Vec<GreedyStep>,
    evaluations: usize,
    report: Option<&SubmodularityReport>,
) -> Result<Selection, EpistemicError> {
    let guarantee = if constraint.is_cardinality_only() {
        theorem::greedy_cardinality(report, true)
    } else {
        theorem::greedy_knapsack()
    };
    Ok(Selection {
        protected: protected.iter().copied().collect(),
        value: function.value(&chosen)?,
        cost: constraint.cost_of(&chosen),
        chosen: chosen.iter().copied().collect(),
        steps,
        guarantee,
        evaluations,
    })
}
