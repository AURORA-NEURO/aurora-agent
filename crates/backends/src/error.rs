//! Typed refusals.
//!
//! Blueprint 43.37 is the reason this module exists rather than a `bool` return: "make the
//! conditions under which FIBER does not compress or accelerate first-class outputs, with
//! standardized fallback reasons". A backend that quietly runs a query it is unsuited to is the
//! failure mode a portfolio is supposed to prevent, so declining is part of the [`crate::QueryBackend`]
//! contract and carries the structural cause, the observed value, and the threshold it crossed.
//!
//! [`RegionError`] is separate on purpose. A malformed region is a caller bug found at
//! construction; a [`Declined`] is a well-formed query this backend should not run.

use bioprism_section::{Backend, FallbackReason};
use thiserror::Error;

/// A backend's refusal to run a query.
///
/// Every variant maps to a stable [`FallbackReason`] so the portfolio can record the refusal in a
/// [`bioprism_section::PlanDescriptor`] without inventing vocabulary.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum Declined {
    /// The induced width of the best order this backend could find exceeds its budget.
    #[error("{backend} declined: induced width {induced_width} exceeds the width budget {budget}")]
    WidthAboveBudget {
        backend: &'static str,
        induced_width: usize,
        budget: usize,
    },

    /// The largest intermediate factor would not fit under the memory limit.
    ///
    /// 43.18 lists memory limits among the *hard* constraints, not among the cost terms, so this
    /// is a refusal rather than a penalty.
    #[error("{backend} declined: peak intermediate of {predicted_entries:.3e} entries exceeds the memory limit of {budget_entries:.3e}")]
    PeakMemoryAboveBudget {
        backend: &'static str,
        predicted_entries: f64,
        budget_entries: f64,
    },

    /// Even the conservative plan exceeds the declared work budget.
    ///
    /// 43.48: "impossible contracts return infeasible, not fabricated output."
    #[error("{backend} declined: predicted {predicted_ops:.3e} semiring operations exceeds the work budget of {budget_ops:.3e}")]
    WorkAboveBudget {
        backend: &'static str,
        predicted_ops: f64,
        budget_ops: f64,
    },

    /// The region carries factor signatures but no potentials, so it can be costed but not run.
    ///
    /// This is the ordinary state of a region sliced out of a `fiber-world/0.1` world: the wire
    /// schema declares each factor's inputs and outputs and never its table. Structural
    /// estimation is defined on signatures alone; execution is not.
    #[error("{backend} declined: factor {factor:?} carries no table; a world-derived region must be given a valuation before it can be executed")]
    MissingFactorTable {
        backend: &'static str,
        factor: String,
    },

    /// The estimate says this backend would not beat the conservative baseline.
    #[error("{backend} declined: predicted cost {predicted_cost:.3e} does not beat direct materialisation at {baseline_cost:.3e} by the required factor {required_speedup}")]
    NoPredictedAdvantage {
        backend: &'static str,
        predicted_cost: f64,
        baseline_cost: f64,
        required_speedup: f64,
    },
}

impl Declined {
    pub fn backend_name(&self) -> &'static str {
        match self {
            Declined::WidthAboveBudget { backend, .. }
            | Declined::PeakMemoryAboveBudget { backend, .. }
            | Declined::WorkAboveBudget { backend, .. }
            | Declined::MissingFactorTable { backend, .. }
            | Declined::NoPredictedAdvantage { backend, .. } => backend,
        }
    }

    /// The stable fallback code this refusal contributes to a plan descriptor.
    ///
    /// Two mappings are lossy and are called out rather than hidden. `fiber-section/0.1` has no
    /// code for *unsupported representation* or for *resource exhaustion*, so
    /// [`Declined::MissingFactorTable`] and [`Declined::WorkAboveBudget`] both land on
    /// [`FallbackReason::BackendExecutionFailure`], and [`Declined::PeakMemoryAboveBudget`] lands
    /// on [`FallbackReason::HighCompiledWidth`] because peak memory is a function of width. The
    /// human-readable detail on the [`bioprism_section::Fallback`] preserves what the code drops.
    pub fn fallback_reason(&self) -> FallbackReason {
        match self {
            Declined::WidthAboveBudget { .. } | Declined::PeakMemoryAboveBudget { .. } => {
                FallbackReason::HighCompiledWidth
            }
            Declined::NoPredictedAdvantage { .. } => FallbackReason::NoPredictedAdvantage,
            Declined::MissingFactorTable { .. } | Declined::WorkAboveBudget { .. } => {
                FallbackReason::BackendExecutionFailure
            }
        }
    }
}

/// A region that cannot be constructed.
///
/// Every variant is an invariant the executor relies on: it indexes tables by a stride computed
/// from the declared cardinalities and accumulates with a semiring whose identities hold only on
/// the non-negative reals. Checking once at construction keeps the inner loop free of guards.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RegionError {
    #[error("factor {factor:?} names variable {variable:?}, which the region does not declare")]
    UnknownVariable { factor: String, variable: String },

    #[error("variable {variable:?} has cardinality zero; a variable with an empty domain makes every factor over it empty")]
    ZeroCardinality { variable: String },

    #[error("factor {factor:?} repeats variable {variable:?} in its scope")]
    RepeatedScopeVariable { factor: String, variable: String },

    #[error("factor {factor:?} appears twice")]
    DuplicateFactorId { factor: String },

    #[error("factor {factor:?} has a table of {actual} entries but its scope requires {expected}")]
    TableSizeMismatch {
        factor: String,
        expected: usize,
        actual: usize,
    },

    #[error("factor {factor:?} has entry {value} at index {index}, which the {semiring} algebra does not admit")]
    InadmissibleTableEntry {
        factor: String,
        index: usize,
        value: f64,
        semiring: &'static str,
    },

    #[error("free variable {variable:?} is not part of the region")]
    FreeVariableNotInRegion { variable: String },

    #[error("materialising a uniform table for factor {factor:?} would need {entries:.3e} entries, above the cap of {cap}")]
    UniformTableTooLarge {
        factor: String,
        entries: f64,
        cap: usize,
    },
}

/// The name a backend reports in a refusal.
pub(crate) fn backend_name(backend: Backend) -> &'static str {
    backend.as_str()
}
