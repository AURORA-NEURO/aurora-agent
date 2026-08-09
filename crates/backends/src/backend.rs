//! The backend contract.
//!
//! Blueprint 43.36 assembles a portfolio out of engines with different eligibility conditions, and
//! its runtime contract requires each to publish a "backend descriptor: supported factors/algebras,
//! exactness, cost features, resources, and policy mode". The load-bearing part of that is
//! eligibility, so it is expressed here as a return type rather than as documentation: both
//! [`QueryBackend::estimate`] and [`QueryBackend::execute`] may return [`Declined`].
//!
//! A backend that runs everything it is handed is the failure mode a portfolio exists to prevent.
//! If elimination cannot fit its intermediates, the useful outcome is a typed refusal naming the
//! width and the budget it crossed — which 43.37 requires the compiler to "expose before
//! execution" — not a slow success or a silently approximated answer.

use crate::dense::DenseFactor;
use crate::error::Declined;
use crate::estimate::Estimate;
use crate::region::QueryRegion;
use crate::semiring::Semiring;
use bioprism_section::Backend;
use serde::{Deserialize, Serialize};

/// One intermediate the executor actually produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntermediateReceipt {
    pub eliminated: Vec<String>,
    pub scope: Vec<String>,
    pub entries: u64,
}

/// 43.19's "execution receipt: order, widths, intermediates, timings, and source witnesses".
///
/// Timings are absent by design — see [`crate::estimate`] for why this crate counts operations
/// rather than seconds. Source witnesses are absent because a region carries factor tables, not the
/// tuples behind them; reconstructing witnesses is 43.20's job and that backend is not implemented
/// here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub backend: Backend,
    pub semiring: Semiring,
    pub order: Vec<String>,
    pub induced_width: Option<usize>,
    pub observed_multiply_ops: u64,
    pub observed_aggregate_ops: u64,
    pub observed_peak_entries: u64,
    pub observed_total_entries: u64,
    pub intermediates: Vec<IntermediateReceipt>,
}

impl ExecutionReceipt {
    pub fn observed_ops(&self) -> u64 {
        self.observed_multiply_ops
            .saturating_add(self.observed_aggregate_ops)
    }
}

/// The answer factor over the query's free variables, plus how it was obtained.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedRegion {
    scope: Vec<String>,
    values: Vec<f64>,
    receipt: ExecutionReceipt,
}

impl ComputedRegion {
    pub(crate) fn new(factor: DenseFactor, receipt: ExecutionReceipt) -> Self {
        ComputedRegion {
            scope: factor.scope,
            values: factor.values,
            receipt,
        }
    }

    /// Free variables, in the order the table is laid out (last varies fastest).
    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }

    pub fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }

    /// The ⊕-aggregate of the whole answer.
    pub fn total(&self, semiring: Semiring) -> f64 {
        self.values
            .iter()
            .fold(semiring.zero(), |left, right| semiring.add(left, *right))
    }

    /// Largest absolute disagreement with another answer over the same scope.
    ///
    /// Returns `None` when the scopes differ, which is a structural disagreement rather than a
    /// numerical one and must not be reported as a small error.
    pub fn max_absolute_difference(&self, other: &ComputedRegion) -> Option<f64> {
        if self.scope != other.scope || self.values.len() != other.values.len() {
            return None;
        }
        Some(
            self.values
                .iter()
                .zip(&other.values)
                .map(|(left, right)| (left - right).abs())
                .fold(0.0, f64::max),
        )
    }

    /// Bit-for-bit agreement.
    ///
    /// Exact equality is the right test when every table entry and every intermediate is an
    /// integer below `2^53`, because then no rounding occurs and reordering the aggregation cannot
    /// change the result. Outside that regime use [`ComputedRegion::max_absolute_difference`].
    pub fn agrees_exactly_with(&self, other: &ComputedRegion) -> bool {
        self.scope == other.scope && self.values == other.values
    }
}

/// A physical engine beneath the factor IR.
pub trait QueryBackend {
    /// The portfolio-level identity recorded in a [`bioprism_section::PlanDescriptor`].
    fn backend(&self) -> Backend;

    /// One line on how this engine decides, so a reader can tell a real backend from a stub.
    fn method(&self) -> String;

    /// Structural prediction, or a typed refusal. Touches no factor table.
    fn estimate(&self, region: &QueryRegion) -> Result<Estimate, Declined>;

    /// Runs the query, or declines it.
    fn execute(&self, region: &QueryRegion) -> Result<ComputedRegion, Declined>;
}
