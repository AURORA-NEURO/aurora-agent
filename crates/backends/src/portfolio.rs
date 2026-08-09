//! Choosing a backend, and recording the choice not to optimize.
//!
//! Blueprint 43.36: `π* = argmin_{π ∈ Π(L_q)} ρ(C(π; θ))` subject to semantic, memory and policy
//! constraints. This module is that argmin over the estimates the backends return, with two
//! properties the blueprint is emphatic about.
//!
//! First, direct materialisation is not one candidate among equals — it is the *baseline every
//! other candidate is measured against*, and the choice to use it is an output. 43.37: "make the
//! conditions under which FIBER does not compress or accelerate first-class outputs, with
//! standardized fallback reasons and non-deceptive performance reporting." A [`Selection`] that
//! lands on [`Backend::DirectMaterialization`] therefore always carries a [`Fallback`] naming the
//! structural cause, and [`Selection::plan_descriptor`] puts it on the wire.
//!
//! Second, a portfolio can be infeasible. If even enumeration exceeds the declared budget,
//! [`Portfolio::select`] returns the refusal rather than picking the least-bad plan — 43.48:
//! "impossible contracts return infeasible, not fabricated output."

use crate::backend::{ComputedRegion, QueryBackend};
use crate::direct::DirectMaterialization;
use crate::elimination::VariableElimination;
use crate::error::Declined;
use crate::estimate::{Budget, CostModel, Estimate};
use crate::order::OrderStrategy;
use crate::region::QueryRegion;
use bioprism_section::{Backend, Fallback, FallbackReason, PlanDescriptor};

/// One backend's answer to "would you run this, and at what cost?".
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub backend: Backend,
    pub method: String,
    pub outcome: Result<Estimate, Declined>,
    /// `ρ(C(π; θ))`, present only when the backend did not decline.
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub chosen: Backend,
    pub method: String,
    pub estimate: Estimate,
    pub cost: f64,
    /// Cost of direct materialisation on the same region.
    pub baseline_cost: f64,
    /// `baseline_cost / cost`. Below `1.0` means the conservative plan was cheaper.
    pub predicted_speedup: f64,
    pub candidates: Vec<Candidate>,
    /// Present exactly when the portfolio settled for the conservative plan.
    pub fallback: Option<Fallback>,
    chosen_index: Option<usize>,
}

impl Selection {
    pub fn is_fallback(&self) -> bool {
        self.fallback.is_some()
    }

    /// The wire-level plan record of 43.36 and 43.37.
    ///
    /// The compiled counts describe the *region*, not the world, which is 43.18's whole point:
    /// `total_factor_count` may be in the hundreds while `compiled_factor_count` is a handful, and
    /// neither number predicts cost on its own.
    pub fn plan_descriptor(&self, region: &QueryRegion) -> PlanDescriptor {
        let provenance = region.provenance();
        PlanDescriptor {
            backend: self.chosen,
            compiled_factor_count: region.factors().len(),
            compiled_fact_count: provenance.compiled_fact_count,
            total_factor_count: provenance.total_factor_count.max(region.factors().len()),
            total_fact_count: provenance.total_fact_count.max(provenance.compiled_fact_count),
            max_selected_factor_arity: region.max_factor_arity(),
            fallback: self.fallback.clone(),
        }
    }
}

pub struct Portfolio {
    backends: Vec<Box<dyn QueryBackend>>,
    baseline: DirectMaterialization,
    cost_model: CostModel,
}

impl Portfolio {
    /// The portfolio this crate actually implements.
    ///
    /// Two elimination candidates, differing only in how they search for an order: exact
    /// minimum-width search where the region is small enough to afford it, and min-degree where it
    /// is not. 43.18 asks for exactly that split — "use exact search for small slices and
    /// heuristics for large slices".
    ///
    /// Four members of [`Backend`] are absent because they are not implemented in this crate;
    /// see the crate documentation for why. A portfolio that registered them as stubs would report
    /// a breadth it does not have.
    pub fn reference() -> Self {
        Portfolio::reference_with(Budget::default())
    }

    /// The reference portfolio under a stated budget, applied to every member.
    ///
    /// Budgets belong to backends rather than to the portfolio because they are eligibility
    /// conditions, not preferences: 43.36's argmin is "subject to semantic, memory, policy,
    /// approximation, and side-effect constraints", and a member that cannot satisfy them is not a
    /// worse candidate but no candidate at all.
    pub fn reference_with(budget: Budget) -> Self {
        Portfolio {
            backends: vec![
                Box::new(
                    VariableElimination::new(OrderStrategy::ExactMinimumWidth).with_budget(budget),
                ),
                Box::new(VariableElimination::new(OrderStrategy::MinDegree).with_budget(budget)),
            ],
            baseline: DirectMaterialization::new().with_budget(budget),
            cost_model: CostModel::default(),
        }
    }

    /// The conservative baseline and nothing else.
    pub fn conservative() -> Self {
        Portfolio::conservative_with(Budget::default())
    }

    pub fn conservative_with(budget: Budget) -> Self {
        Portfolio {
            backends: Vec::new(),
            baseline: DirectMaterialization::new().with_budget(budget),
            cost_model: CostModel::default(),
        }
    }

    pub fn with_cost_model(mut self, cost_model: CostModel) -> Self {
        self.cost_model = cost_model;
        self
    }

    pub fn push(&mut self, backend: Box<dyn QueryBackend>) {
        self.backends.push(backend);
    }

    pub fn cost_model(&self) -> CostModel {
        self.cost_model
    }

    /// Estimates every candidate and returns the plan, or the reason there is none.
    ///
    /// The baseline may itself decline — a region whose joint space exceeds the work budget cannot
    /// be enumerated — and that does not by itself make the region infeasible: an optimizing
    /// backend narrow enough to fit is then the only plan, and it is chosen without a margin
    /// because there is nothing to compare it against. Infeasibility is when *nothing* qualifies.
    pub fn select(&self, region: &QueryRegion) -> Result<Selection, Declined> {
        let baseline_outcome = self.baseline.estimate(region);
        let baseline_cost = baseline_outcome
            .as_ref()
            .ok()
            .map(|estimate| self.cost_model.cost(estimate));

        let mut candidates: Vec<Candidate> = Vec::with_capacity(self.backends.len() + 1);
        let mut best: Option<(usize, f64)> = None;

        for (index, backend) in self.backends.iter().enumerate() {
            let outcome = backend.estimate(region);
            let cost = outcome.as_ref().ok().map(|e| self.cost_model.cost(e));
            if let Some(cost) = cost {
                if best.is_none_or(|(_, incumbent)| cost < incumbent) {
                    best = Some((index, cost));
                }
            }
            candidates.push(Candidate {
                backend: backend.backend(),
                method: backend.method(),
                outcome,
                cost,
            });
        }

        candidates.push(Candidate {
            backend: self.baseline.backend(),
            method: self.baseline.method(),
            outcome: baseline_outcome.clone(),
            cost: baseline_cost,
        });

        let reported_baseline_cost = baseline_cost.unwrap_or(f64::INFINITY);

        if let Some((index, cost)) = best {
            let worth_switching = match baseline_cost {
                None => true,
                Some(baseline) => cost * self.cost_model.minimum_speedup <= baseline,
            };
            if worth_switching {
                let estimate = candidates[index]
                    .outcome
                    .as_ref()
                    .expect("a costed candidate did not decline")
                    .clone();
                return Ok(Selection {
                    chosen: estimate.backend,
                    method: estimate.method.clone(),
                    estimate,
                    cost,
                    baseline_cost: reported_baseline_cost,
                    predicted_speedup: speedup(reported_baseline_cost, cost),
                    candidates,
                    fallback: None,
                    chosen_index: Some(index),
                });
            }
        }

        let baseline_estimate = baseline_outcome?;
        let fallback = fallback_for(
            &candidates,
            best,
            reported_baseline_cost,
            self.cost_model.minimum_speedup,
        );
        Ok(Selection {
            chosen: self.baseline.backend(),
            method: self.baseline.method(),
            estimate: baseline_estimate,
            cost: reported_baseline_cost,
            baseline_cost: reported_baseline_cost,
            predicted_speedup: 1.0,
            candidates,
            fallback: Some(fallback),
            chosen_index: None,
        })
    }

    /// Selects, then runs. A backend that declines at execution time is replaced by the
    /// conservative plan and the substitution is recorded, per 43.36's "fallback chain".
    pub fn execute(
        &self,
        region: &QueryRegion,
    ) -> Result<(ComputedRegion, Selection), Declined> {
        let mut selection = self.select(region)?;
        if let Some(index) = selection.chosen_index {
            match self.backends[index].execute(region) {
                Ok(computed) => return Ok((computed, selection)),
                Err(declined) => {
                    selection.chosen = self.baseline.backend();
                    selection.method = self.baseline.method();
                    selection.chosen_index = None;
                    selection.cost = selection.baseline_cost;
                    selection.predicted_speedup = 1.0;
                    selection.fallback = Some(Fallback {
                        reason: FallbackReason::BackendExecutionFailure,
                        detail: format!(
                            "{declined}; re-ran the same logical query under direct materialisation"
                        ),
                    });
                }
            }
        }
        let computed = self.baseline.execute(region)?;
        Ok((computed, selection))
    }
}

fn speedup(baseline: f64, cost: f64) -> f64 {
    if cost <= 0.0 {
        1.0
    } else {
        baseline / cost
    }
}

fn fallback_for(
    candidates: &[Candidate],
    best: Option<(usize, f64)>,
    baseline_cost: f64,
    minimum_speedup: f64,
) -> Fallback {
    if let Some((index, cost)) = best {
        return Fallback {
            reason: FallbackReason::NoPredictedAdvantage,
            detail: format!(
                "best optimizing candidate {} costs {cost:.3e} against a direct-materialisation baseline of {baseline_cost:.3e}; the {minimum_speedup}x margin required before switching plans was not met",
                candidates[index].backend.as_str()
            ),
        };
    }

    match candidates.iter().find_map(|c| c.outcome.as_ref().err()) {
        Some(declined) => Fallback {
            reason: declined.fallback_reason(),
            detail: declined.to_string(),
        },
        None => Fallback {
            reason: FallbackReason::NoPredictedAdvantage,
            detail: "the portfolio registers no optimizing backend for this region".to_string(),
        },
    }
}
