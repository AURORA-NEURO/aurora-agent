//! Variable elimination: the one real compute backend in this crate.
//!
//! Blueprint 43.19 makes this the default exact engine — "use a functional aggregate query backend
//! as the default exact engine when the logical program factorizes over compatible algebras and a
//! favorable elimination order exists" — and states the reason it can win: "runtime is controlled
//! by query width and input representation, not merely factor count."
//!
//! The implementation is InsideOut-style bucket elimination. Each bound variable in turn collects
//! the factors mentioning it, multiplies them, aggregates the variable away, and puts the resulting
//! message back into the pool. Correctness rests entirely on distributivity: a factor that does not
//! mention the eliminated variable can be pulled outside the aggregation, which is why
//! [`crate::Semiring`] admits only commutative semirings.
//!
//! ## What is here and what is not
//!
//! Present: the exact engine, the order search, the width and cost estimate, and a typed refusal
//! when the plan does not fit. Absent, and deliberately: separator-message caching across related
//! queries (43.24), federated message passing (43.19's fifth fallback), and log-domain algebra for
//! numerical underflow. The first two need infrastructure this crate does not own; the third is a
//! real gap for long products of small probabilities and is recorded rather than papered over,
//! because a silently underflowing sum-product returns zero instead of failing.

use crate::backend::{ComputedRegion, QueryBackend};
use crate::error::{backend_name, Declined};
use crate::estimate::{Budget, Estimate};
use crate::execute;
use crate::order::{elimination_order, EliminationOrder, OrderStrategy, WidthMetric};
use crate::region::QueryRegion;
use crate::schedule::{bucket_schedule, Schedule};
use bioprism_section::Backend;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariableElimination {
    pub strategy: OrderStrategy,
    pub budget: Budget,
}

impl Default for VariableElimination {
    fn default() -> Self {
        VariableElimination {
            strategy: OrderStrategy::MinFill,
            budget: Budget::default(),
        }
    }
}

impl VariableElimination {
    pub fn new(strategy: OrderStrategy) -> Self {
        VariableElimination {
            strategy,
            budget: Budget::default(),
        }
    }

    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    /// The order and the schedule the estimate describes and the executor runs.
    pub fn plan(&self, region: &QueryRegion) -> (EliminationOrder, Schedule) {
        let order = elimination_order(region, self.strategy);
        let schedule = bucket_schedule(region, &order);
        (order, schedule)
    }
}

impl QueryBackend for VariableElimination {
    fn backend(&self) -> Backend {
        Backend::FaqInsideOut
    }

    fn method(&self) -> String {
        format!(
            "InsideOut bucket elimination over the factor graph, {} elimination order",
            self.strategy.as_str()
        )
    }

    fn estimate(&self, region: &QueryRegion) -> Result<Estimate, Declined> {
        let name = backend_name(self.backend());
        self.budget
            .validate()
            .map_err(|detail| Declined::InvalidConfiguration {
                backend: name,
                detail,
            })?;
        let (order, schedule) = self.plan(region);

        if order.induced_width > self.budget.max_induced_width {
            return Err(Declined::WidthAboveBudget {
                backend: name,
                induced_width: order.induced_width,
                budget: self.budget.max_induced_width,
            });
        }
        let peak = schedule.peak_entries();
        if peak > self.budget.max_peak_entries {
            return Err(Declined::PeakMemoryAboveBudget {
                backend: name,
                predicted_entries: peak,
                budget_entries: self.budget.max_peak_entries,
            });
        }
        let ops = schedule.total_ops();
        if ops > self.budget.max_ops {
            return Err(Declined::WorkAboveBudget {
                backend: name,
                predicted_ops: ops,
                budget_ops: self.budget.max_ops,
            });
        }

        let mut assumptions = region.assumptions().to_vec();
        assumptions.push(format!(
            "induced width {} is from the {} order; finding the minimum-width order is NP-hard and this bound is {}",
            order.induced_width,
            order.used.as_str(),
            match order.bound {
                crate::order::Bound::Exact => "the exact optimum for this region",
                crate::order::Bound::HeuristicUpperBound => "an upper bound on the region's treewidth",
            }
        ));

        let estimate = Estimate {
            backend: self.backend(),
            method: self.method(),
            width_metric: WidthMetric::InducedWidth,
            induced_width: Some(order.induced_width),
            bound: order.bound,
            order: order.order,
            predicted_multiply_ops: schedule.multiply_ops(),
            predicted_aggregate_ops: schedule.aggregate_ops(),
            predicted_peak_entries: peak,
            predicted_total_entries: schedule.total_entries(),
            uncertainty: region.assumed_cardinality_fraction(),
            assumptions,
        };
        estimate
            .validate()
            .map_err(|detail| Declined::InvalidEstimate {
                backend: name,
                detail,
            })?;
        Ok(estimate)
    }

    fn execute(&self, region: &QueryRegion) -> Result<ComputedRegion, Declined> {
        self.estimate(region)?;
        let (order, schedule) = self.plan(region);
        execute::run(
            region,
            &schedule,
            self.backend(),
            order.order,
            Some(order.induced_width),
            &self.budget,
        )
    }
}
