#![allow(ambiguous_glob_reexports)]

//! Generated projections of compiled decision regions.
//!
//! Implements blueprint section 41 (graph-first knowledge and navigation) and section 42
//! (graph-native evaluation hub and UI) **under the constraint of 43.01**, which is the decision
//! this crate exists to obey:
//!
//! > Use a query-compiled fibered factor calculus as the canonical inference substrate. Graphs,
//! > hypergraphs, timelines, tables, and narratives are generated projections for navigation,
//! > visualization, interoperability, and debugging.
//!
//! Sections 41 and 42 were written when the graph *was* the substrate, and both now carry a v0.6
//! clarification saying it is not: 41.00 states that graphs are compiled projections, and 42.00
//! states that hub views "are generated from Decision Sections and Context Certificates". This
//! crate is the code path that makes that true rather than merely stated.
//!
//! # What a projection is here
//!
//! A [`Projection`] takes a [`DecisionSection`](bioprism_section::DecisionSection) and a
//! [`ProjectionSource`] and returns a [`View`]. The source is not decoration: it holds the world,
//! query, section and certificate digests, it is computed by hashing rather than asserted, and it
//! refuses to bind a certificate that attests a different section. A `View` has private fields and
//! a crate-private constructor, so outside this crate there is no route to one except through
//! `project`. Building a view without the provenance it came from is not discouraged; it does not
//! typecheck.
//!
//! # What is deliberately absent
//!
//! **No relevance parameter.** 43.01: "Completeness is defined against query obligations and
//! protected closure, not neighborhood radius." Nothing in this crate's public API takes a
//! `k_hop`, `depth`, `radius`, `max_nodes` or token budget, because any such argument would be a
//! second relevance policy operating after the compiler already applied protected closure — and
//! an unaudited one, since it would not appear in the Context Certificate. A caller who wants a
//! smaller view compiles a smaller region.
//!
//! **No rendering.** There is no layout, no coordinates, no colour, no HTML, no web UI and no
//! server here. This crate produces the data a renderer would draw. That is also why every state
//! is a typed field — 40.29 requires that "color is never the only encoding", and a data layer
//! that never emits colour cannot break that rule.
//!
//! **No storage.** Nothing here is a persistence format. The hypergraph view materialises
//! incidence because a hypergraph rendering needs it, and says so inside its own payload, because
//! 43.01's whole argument is that explicit global incidence is not what makes a context correct.
//!
//! # The four projections
//!
//! | Module | View | Loses |
//! |---|---|---|
//! | [`graph`] | typed, directed nodes and edges over the normative 41.03 vocabulary | joint semantics of multiway factors; evidence values |
//! | [`hypergraph`] | factors kept whole as hyperedges — the inspector 43.01 requires alongside the graph | what each factor's constraint actually *is* |
//! | [`timeline`] | the causal event structure on a line, event time never merged with availability time | concurrency, wherever the line imposes an order |
//! | [`table`] | the accessible fallback of 40.29 and 42.27 | typed edges, flattened to text |
//!
//! # The one loss that is forbidden
//!
//! A view may flatten anything except an obstruction. Every projection must carry every unresolved
//! obligation and every oracle witness into its body; [`FidelityLedger`] refuses to seal otherwise,
//! and the render fails with a typed error. A smaller picture is a projection. A picture missing
//! the reason the decision is blocked is a misrepresentation, and 43.01's first invariant — "no
//! visualization is treated as proof of evidence completeness" — is only enforceable if that case
//! is an error rather than a style preference.
//!
//! # Not implemented
//!
//! - **Policy-aware projection.** 42.01 requires views to be filtered by access policy and to
//!   carry a `PolicyDecision`. That belongs to the policy fibers of 43.33 and is not modelled
//!   here; these projections render whatever the compiled region contains.
//! - **Aggregation of high-cardinality nodes.** 42.01 asks for certified quotient aggregation of
//!   cell, voxel and variant nodes, with exact handles retained for aggregated members. 43.10's
//!   decision-equivalence quotients are the certification that would make it safe, and until they
//!   exist an aggregation here would be the "arbitrary clustering" 43.01 explicitly rejects.
//! - **Deep links, saved views, receipts.** 42.25 and 42.26 are hub surface, not projection data.
//! - **Amendment and supersession edges.** See [`vocabulary`].

pub mod error;
pub mod factors;
pub mod federated_continual_projection_integrity_contract_model;
pub mod federated_continual_projection_integrity_inference;
pub mod federated_continual_projection_integrity_research_copilot;
pub mod federated_continual_projection_integrity_workflow_fabric;
pub mod fidelity;
pub mod graph;
pub mod hypergraph;
pub mod identity;
pub mod lint;
pub mod local_projection_integrity_contract_model;
pub mod local_projection_integrity_inference;
pub mod local_projection_integrity_research_copilot;
pub mod local_projection_integrity_workflow_fabric;
pub mod markers;
pub mod multimodal_projection_integrity_contract_model;
pub mod multimodal_projection_integrity_inference;
pub mod multimodal_projection_integrity_research_copilot;
pub mod multimodal_projection_integrity_workflow_fabric;
pub mod projection_integrity_support;
pub mod provenance;
pub mod roundtrip;
pub mod table;
pub mod throughput_projection_integrity_contract_model;
pub mod throughput_projection_integrity_inference;
pub mod throughput_projection_integrity_research_copilot;
pub mod throughput_projection_integrity_workflow_fabric;
pub mod timeline;
pub mod view;
pub mod vocabulary;

pub use error::ProjectionError;
pub use factors::{selected_factors, SelectedFactor};
pub use federated_continual_projection_integrity_contract_model::*;
pub use federated_continual_projection_integrity_inference::*;
pub use federated_continual_projection_integrity_research_copilot::*;
pub use federated_continual_projection_integrity_workflow_fabric::*;
pub use fidelity::{DropReason, DroppedFeature, FidelityLedger, FidelityReport};
pub use graph::{
    EdgeGloss, GraphBody, GraphEdge, GraphNode, GraphProjection, MultiwayNote, NotEmitted,
};
pub use hypergraph::{
    HyperVertex, Hyperedge, HypergraphBody, HypergraphProjection, Pin, PinRole, RENDERING_NOTE,
};
pub use lint::{lint_graph, GraphLint};
pub use local_projection_integrity_contract_model::*;
pub use local_projection_integrity_inference::*;
pub use local_projection_integrity_research_copilot::*;
pub use local_projection_integrity_workflow_fabric::*;
pub use markers::{ConflictMarker, ObligationMarker};
pub use multimodal_projection_integrity_contract_model::*;
pub use multimodal_projection_integrity_inference::*;
pub use multimodal_projection_integrity_research_copilot::*;
pub use multimodal_projection_integrity_workflow_fabric::*;
pub use projection_integrity_support::*;
pub use provenance::{BoundSection, ProjectionSource, ProvenanceCheck};
pub use roundtrip::{
    evidence_survives, obstructions_survive, project_all, HandleCoverage, ProjectionBundle,
};
pub use table::{TableBody, TableProjection, TableRow, COLUMNS};
pub use throughput_projection_integrity_contract_model::*;
pub use throughput_projection_integrity_inference::*;
pub use throughput_projection_integrity_research_copilot::*;
pub use throughput_projection_integrity_workflow_fabric::*;
pub use timeline::{
    Availability, ClockAnomaly, OrderJustification, TimelineAxis, TimelineBody, TimelineEntry,
    TimelineProjection,
};
pub use view::{ProjectRegion, ProjectedBody, Projection, ProjectionKind, View};
pub use vocabulary::{EdgeType, NodeKind, NodeStatus};
pub mod computational_execution_gateway;
pub use computational_execution_gateway::{
    admit as admit_computational_execution,
    capability_manifest as computational_execution_capability_manifest, ExecutionNode,
    ExecutionRun, GatewayError, ResearchWorkflowSpec,
    CONTRACT_VERSION as COMPUTATIONAL_EXECUTION_CONTRACT_VERSION,
    FEATURE_ID as COMPUTATIONAL_EXECUTION_FEATURE_ID,
    INPUT_SCHEMA as COMPUTATIONAL_EXECUTION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as COMPUTATIONAL_EXECUTION_OUTPUT_SCHEMA,
};
