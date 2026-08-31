//! The physical backend portfolio.
//!
//! Implements blueprint 43.18 (elimination order, compiled width and cost model), 43.19 (functional
//! aggregate query / InsideOut / generalized distributive law), 43.36 (portfolio optimizer),
//! 43.37 (failure modes, limits and explicit fallbacks) and the phase-boundary measurement of
//! 43.48. It sits beneath the compiler: everything here is costed and run from a
//! [`QueryRegion`] alone, with no query, no policy and no compiler in the link.
//!
//! ## The claim this crate exists to make measurable
//!
//! 43.18: "never use total graph size as the primary complexity predictor." The reference world in
//! `fixtures/` holds 761 facts and 756 factors; the split-integrity query reaches six factors over
//! seventeen variables and eliminates them at induced width five. Two queries against that same
//! world can differ by orders of magnitude in cost while the world does not change at all. This
//! crate computes the query-specific statistic — the width of a real elimination order over the
//! region's primal graph — and prices it.
//!
//! ## Structure
//!
//! - [`region`] — what one query reaches, with domain cardinalities and, optionally, potentials.
//! - [`order`] — the primal graph, min-degree and min-fill orders, exact minimum-width search on
//!   small regions, and the induced width each order produces.
//! - [`schedule`] — the plan both backends execute, so estimate and execution describe the same
//!   traversal.
//! - [`elimination`] — InsideOut bucket elimination. The one real compute engine here.
//! - [`direct`] — streaming enumeration: the conservative plan, and the brute-force oracle.
//! - [`portfolio`] — the argmin of 43.36, and the [`bioprism_section::Fallback`] it records when it
//!   declines to optimize.
//! - [`phase`] — a width-controlled sweep that measures where elimination stops paying.
//!
//! ## Declining is part of the contract
//!
//! [`QueryBackend::estimate`] and [`QueryBackend::execute`] both return `Result<_, `[`Declined`]`>`.
//! A backend that runs whatever it is handed is precisely the failure a portfolio exists to
//! prevent, so refusal is typed, carries the threshold it crossed, and maps to a stable
//! [`bioprism_section::FallbackReason`]. 43.37: "fallback is not counted as optimized success" —
//! and equally, it is not counted as failure. A high-width query answered by enumeration was
//! answered correctly.
//!
//! ## What is not implemented, and why
//!
//! [`bioprism_section::Backend`] names seven engines. Two are implemented here. The others are
//! absent rather than stubbed, because a portfolio containing stubs reports a breadth it does not
//! have:
//!
//! - **Worst-case-optimal joins (43.20).** Needs relational factors — trie-indexed tuples with
//!   seek/open/next iterators. The `fiber-world/0.1` schema declares a factor's signature and its
//!   kind, never its extension, so there is nothing to build a trie over. This is a schema gap, not
//!   an effort gap.
//! - **Tensor networks and low-rank contraction (43.21).** The representation's whole argument is
//!   that a truncated decomposition beats dense contraction, which needs a competitive dense kernel
//!   and a truncated SVD — a BLAS/LAPACK-class dependency reached through C FFI. A pure-Rust
//!   reimplementation would lose to the library by a large constant, and 43.38 forbids comparing
//!   against an under-engineered baseline; a benchmark built on one would be worse than no
//!   benchmark.
//! - **Decision diagrams (43.22).** A BDD/ZDD engine's behaviour is dominated by its unique table,
//!   apply cache and dynamic reordering. Those live in CUDD or Sylvan, again through C FFI. A naive
//!   Rust BDD would exercise none of the properties 43.22 claims and would produce a phase diagram
//!   about the implementation rather than the representation.
//! - **Incremental view maintenance (43.23).** Needs the separator cache of 43.24 and a change
//!   stream, neither of which this crate owns.
//!
//! ## What the numbers mean
//!
//! Cost is counted in semiring operations, never in seconds, and no part of this crate claims a
//! latency result. See [`estimate`] for why, and [`phase`] for what that does and does not license
//! a reader to conclude.

pub mod backend;
pub mod direct;
pub mod elimination;
pub mod error;
pub mod estimate;
pub mod federated_retrieval_synthesis_workflow_fabric;
pub mod capability_negotiation_integrity_support;
pub mod local_capability_negotiation_integrity_inference;
pub mod multimodal_capability_negotiation_integrity_inference;
pub mod throughput_capability_negotiation_integrity_inference;
pub mod federated_continual_capability_negotiation_integrity_inference;
pub mod local_capability_negotiation_integrity_contract_model;
pub mod multimodal_capability_negotiation_integrity_contract_model;
pub mod throughput_capability_negotiation_integrity_contract_model;
pub mod federated_continual_capability_negotiation_integrity_contract_model;
pub mod local_capability_negotiation_integrity_research_copilot;
pub mod multimodal_capability_negotiation_integrity_research_copilot;
pub mod throughput_capability_negotiation_integrity_research_copilot;
pub mod federated_continual_capability_negotiation_integrity_research_copilot;
pub mod local_capability_negotiation_integrity_workflow_fabric;
pub mod multimodal_capability_negotiation_integrity_workflow_fabric;
pub mod throughput_capability_negotiation_integrity_workflow_fabric;
pub mod federated_continual_capability_negotiation_integrity_workflow_fabric;
pub mod order;
pub mod phase;
pub mod portfolio;
pub mod region;
pub mod schedule;
pub mod semiring;

mod dense;
mod execute;

pub use backend::{ComputedRegion, ExecutionReceipt, IntermediateReceipt, QueryBackend};
pub use direct::DirectMaterialization;
pub use elimination::VariableElimination;
pub use error::{Declined, RegionError};
pub use estimate::{Budget, CostModel, Estimate};
pub use federated_retrieval_synthesis_workflow_fabric::{
    federated_retrieval_synthesis_workflow_manifest, run_federated_retrieval_synthesis,
    FederatedRetrievalArtifact8, FederatedRetrievalDisposition, FederatedRetrievalError,
    FederatedRetrievalSynthesisRequest6, FederatedRetrievalSynthesisRun8, RetrievalPeer5,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_SYNTHESIS_WORKFLOW_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_SYNTHESIS_WORKFLOW_FEATURE_ID,
};
pub use capability_negotiation_integrity_support::{
    BackendArtifact4, BackendCandidate4, BackendCard7, BackendRequest4,
    CapabilityNegotiationIntegrityError,
};
pub use order::{
    elimination_order, Bound, EliminationClique, EliminationOrder, OrderStrategy, PrimalGraph,
    WidthMetric, EXACT_ORDER_MAX_VARIABLES,
};
pub use phase::{band_region, sweep_induced_width, PhaseDiagram, PhasePoint};
pub use portfolio::{Candidate, Portfolio, Selection};
pub use region::{
    CardinalityPolicy, CardinalitySource, QueryRegion, QueryRegionBuilder, RegionFactor,
    RegionProvenance, MAX_MATERIALISED_TABLE_ENTRIES,
};
pub use schedule::{bucket_schedule, direct_schedule, Schedule, Step, StepInput};
pub use semiring::Semiring;
