#![allow(clippy::all)]

//! Equal-engineering context baselines.
//!
//! Implements the comparative evaluation program of blueprint 43.38 and the baseline panel named
//! in 43.41: full files, vector top-k, graph k-hop, query graph, hypergraph component, and FIBER.
//!
//! The crate exists so that the central claim is *falsifiable*. Without competent baselines,
//! "FIBER compiles a smaller context" is unfalsifiable marketing; with them it is a measurement
//! that can come out the other way — and if a graph baseline stays compact under equal
//! optimisation, 43.41 requires reporting that result rather than burying it.

pub mod compare;
pub mod counterfactual_integrity_support;
pub mod directed;
pub mod embedding;
pub mod federated_continual_counterfactual_integrity_contract_model;
pub mod federated_continual_counterfactual_integrity_inference;
pub mod federated_continual_counterfactual_integrity_research_copilot;
pub mod federated_continual_counterfactual_integrity_workflow_fabric;
pub mod incidence;
pub mod index;
pub mod lexical;
pub mod local_counterfactual_integrity_contract_model;
pub mod local_counterfactual_integrity_inference;
pub mod local_counterfactual_integrity_research_copilot;
pub mod local_counterfactual_integrity_workflow_fabric;
pub mod multimodal_counterfactual_integrity_contract_model;
pub mod multimodal_counterfactual_integrity_inference;
pub mod multimodal_counterfactual_integrity_research_copilot;
pub mod multimodal_counterfactual_integrity_workflow_fabric;
pub mod strategy;
pub mod sweep;
pub mod throughput_counterfactual_integrity_contract_model;
pub mod throughput_counterfactual_integrity_inference;
pub mod throughput_counterfactual_integrity_research_copilot;
pub mod throughput_counterfactual_integrity_workflow_fabric;

pub use compare::{
    compare, default_panel, extended_panel, CompareError, Comparison, Judgement, RowRefusal,
    RowVerdict, StrategyResult,
};
pub use counterfactual_integrity_support::{
    manifest as counterfactual_integrity_manifest, qualify as qualify_counterfactual_integrity,
    BaselineArm4, CounterfactualIntegrityArtifact4, CounterfactualIntegrityCard7,
    CounterfactualIntegrityError, CounterfactualIntegrityRequest4,
    BOUNDARY as COUNTERFACTUAL_INTEGRITY_BOUNDARY,
    CONTENT_TYPE as COUNTERFACTUAL_INTEGRITY_CONTENT_TYPE,
};
pub use directed::{DirectedDependencyWalk, EngineeredPasses, ScreenedDependencyWalk};
pub use embedding::EmbeddingTopK;
pub use federated_continual_counterfactual_integrity_contract_model::*;
pub use federated_continual_counterfactual_integrity_inference::*;
pub use federated_continual_counterfactual_integrity_research_copilot::*;
pub use federated_continual_counterfactual_integrity_workflow_fabric::*;
pub use incidence::{ConnectedComponent, KHopIncidence, QueryGraph};
pub use index::PanelIndex;
pub use lexical::LexicalTopK;
pub use local_counterfactual_integrity_contract_model::*;
pub use local_counterfactual_integrity_inference::*;
pub use local_counterfactual_integrity_research_copilot::*;
pub use local_counterfactual_integrity_workflow_fabric::*;
pub use multimodal_counterfactual_integrity_contract_model::*;
pub use multimodal_counterfactual_integrity_inference::*;
pub use multimodal_counterfactual_integrity_research_copilot::*;
pub use multimodal_counterfactual_integrity_workflow_fabric::*;
pub use strategy::{ContextStrategy, FiberCompiled, FullContext, Selection};
pub use sweep::{
    run_cell, run_sweep, sweep_panel, SweepCell, SweepError, SweepGrid, SweepRow, SweepTable,
};
pub use throughput_counterfactual_integrity_contract_model::*;
pub use throughput_counterfactual_integrity_inference::*;
pub use throughput_counterfactual_integrity_research_copilot::*;
pub use throughput_counterfactual_integrity_workflow_fabric::*;
pub mod interpretation_assurance;
pub use interpretation_assurance::{
    assure_multimodal_interpretation, interpretation_assurance_manifest,
    EvidenceBackedResult as BaselineEvidenceBackedResult,
    InterpretationAssuranceError as BaselineInterpretationAssuranceError,
    InterpretationAssuranceReceipt as BaselineInterpretationAssuranceReceipt,
    InterpretationAssuranceRequest as BaselineInterpretationAssuranceRequest,
    InterpretationDisposition as BaselineInterpretationDisposition,
    CONTRACT_VERSION as BASELINE_INTERPRETATION_CONTRACT_VERSION,
    FEATURE_ID as BASELINE_INTERPRETATION_FEATURE_ID,
};
