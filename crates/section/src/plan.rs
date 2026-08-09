//! Physical plan description.
//!
//! Blueprint 43.36 selects a backend from a portfolio and 43.37 requires the conditions under
//! which FIBER *does not* compress or accelerate to be first-class outputs. A plan therefore
//! always names its backend and always carries an explicit [`Fallback`] field — `None` when the
//! chosen backend ran, `Some` when the compiler declined to compress and says why.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// The v0.1 reference engine: reachability-based backward slicing over the factor graph.
    BackwardFactorSliceReference,
    /// Functional aggregate query / InsideOut / generalized distributive law.
    FaqInsideOut,
    /// Worst-case-optimal multiway joins.
    WorstCaseOptimalJoin,
    /// Sparse or low-rank tensor contraction.
    TensorNetwork,
    /// BDD / ZDD / algebraic decision diagram.
    DecisionDiagram,
    /// Incremental view maintenance over separator caches.
    IncrementalView,
    /// Bounded direct materialisation. Not a failure: the honest answer when no compression is
    /// justified by the query's structure.
    DirectMaterialization,
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::BackwardFactorSliceReference => "backward_factor_slice_reference",
            Backend::FaqInsideOut => "faq_inside_out",
            Backend::WorstCaseOptimalJoin => "worst_case_optimal_join",
            Backend::TensorNetwork => "tensor_network",
            Backend::DecisionDiagram => "decision_diagram",
            Backend::IncrementalView => "incremental_view",
            Backend::DirectMaterialization => "direct_materialization",
        }
    }
}

/// Why the compiler fell back instead of using a compressing backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    /// Compiled width exceeded the budget at which factorisation pays.
    HighCompiledWidth,
    /// Protected closure alone already covers most of the world, so there is nothing to omit.
    ProtectedClosureDominates,
    /// Policy fragmentation left no reusable separator.
    PolicyFragmentation,
    /// The structural estimate predicted no advantage.
    NoPredictedAdvantage,
    /// A backend failed and the plan was re-run without changing logical semantics.
    BackendExecutionFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fallback {
    pub reason: FallbackReason,
    pub detail: String,
}

/// Structural statistics reported alongside the plan.
///
/// 43.18 is explicit that total graph size is *not* the primary complexity predictor, so these
/// are query-specific: what the slice actually touched, not how big the world is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanDescriptor {
    pub backend: Backend,
    pub compiled_factor_count: usize,
    pub compiled_fact_count: usize,
    pub total_factor_count: usize,
    pub total_fact_count: usize,
    pub max_selected_factor_arity: usize,
    pub fallback: Option<Fallback>,
}

impl PlanDescriptor {
    /// Fraction of the world's facts that survived compilation. Reported, never asserted to be
    /// small: a high-width query legitimately keeps most of the world.
    pub fn fact_selection_ratio(&self) -> f64 {
        if self.total_fact_count == 0 {
            return 1.0;
        }
        self.compiled_fact_count as f64 / self.total_fact_count as f64
    }

    pub fn factor_selection_ratio(&self) -> f64 {
        if self.total_factor_count == 0 {
            return 1.0;
        }
        self.compiled_factor_count as f64 / self.total_factor_count as f64
    }
}
