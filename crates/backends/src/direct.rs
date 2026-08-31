//! Direct materialisation: the conservative plan, and the reference answer.
//!
//! Blueprint 43.36 names direct materialisation as the choice when "no eligible optimized plan"
//! exists, and 43.37 insists it is a first-class outcome rather than a defeat: "fallback is not
//! counted as optimized success" — but neither is it counted as failure. A query whose structure
//! offers nothing to exploit is correctly answered by enumeration, and saying so is the honest
//! report.
//!
//! Two roles, one implementation. As a portfolio member it is the plan of last resort. As a test
//! oracle it is the brute-force answer variable elimination is checked against, which only works
//! because it shares the kernel and the schedule representation with elimination and differs solely
//! in aggregating every bound variable in a single pass. 43.38 forbids an under-engineered
//! baseline, so this one streams the joint assignment space rather than allocating it: its memory
//! is the answer table, and elimination has to win on time alone.

use crate::backend::{ComputedRegion, QueryBackend};
use crate::error::{backend_name, Declined};
use crate::estimate::{Budget, Estimate};
use crate::execute;
use crate::order::{Bound, WidthMetric};
use crate::region::QueryRegion;
use crate::schedule::{direct_schedule, Schedule};
use bioprism_section::Backend;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DirectMaterialization {
    pub budget: Budget,
}

impl DirectMaterialization {
    pub fn new() -> Self {
        DirectMaterialization::default()
    }

    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    pub fn plan(&self, region: &QueryRegion) -> Schedule {
        direct_schedule(region)
    }
}

impl QueryBackend for DirectMaterialization {
    fn backend(&self) -> Backend {
        Backend::DirectMaterialization
    }

    fn method(&self) -> String {
        "single streaming pass over the joint assignment space, aggregating every bound variable at once".to_string()
    }

    fn estimate(&self, region: &QueryRegion) -> Result<Estimate, Declined> {
        let name = backend_name(self.backend());
        self.budget
            .validate()
            .map_err(|detail| Declined::InvalidConfiguration {
                backend: name,
                detail,
            })?;
        let schedule = self.plan(region);

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
        assumptions.push(
            "cost is the full joint assignment space; no structural property of the region reduces it"
                .to_string(),
        );

        let estimate = Estimate {
            backend: self.backend(),
            method: self.method(),
            width_metric: WidthMetric::NotApplicable,
            induced_width: None,
            bound: Bound::Exact,
            order: Vec::new(),
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
        let schedule = self.plan(region);
        execute::run(
            region,
            &schedule,
            self.backend(),
            Vec::new(),
            None,
            &self.budget,
        )
    }
}
