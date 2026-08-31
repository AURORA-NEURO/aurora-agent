//! The FIBER world model.
//!
//! Implements the data half of blueprint 43.02 (formal object), 43.04 (evidence fibration and
//! local sections), 43.07 (factorized evidence algebra) and 43.09 (causal event structures),
//! against the `fiber-world/0.1` wire schema.
//!
//! This crate is deliberately inert: it parses, indexes and diagnoses. Every decision about
//! *which* of these objects a query needs belongs to the compiler, so that the world can be
//! shared unchanged across queries, roles and policies.

#![allow(clippy::all)]

pub mod causal_integrity_support;
pub mod error;
pub mod event;
pub mod fact;
pub mod factor;
pub mod federated_continual_causal_integrity_contract_model;
pub mod federated_continual_causal_integrity_inference;
pub mod federated_continual_causal_integrity_research_copilot;
pub mod federated_continual_causal_integrity_workflow_fabric;
pub mod index;
pub(crate) mod json;
pub mod local_causal_integrity_contract_model;
pub mod local_causal_integrity_inference;
pub mod local_causal_integrity_research_copilot;
pub mod local_causal_integrity_workflow_fabric;
pub mod multimodal_causal_integrity_contract_model;
pub mod multimodal_causal_integrity_inference;
pub mod multimodal_causal_integrity_research_copilot;
pub mod multimodal_causal_integrity_workflow_fabric;
pub mod source;
pub mod throughput_causal_integrity_contract_model;
pub mod throughput_causal_integrity_inference;
pub mod throughput_causal_integrity_research_copilot;
pub mod throughput_causal_integrity_workflow_fabric;
pub mod validate;
pub mod world;

pub use causal_integrity_support::{
    manifest as causal_integrity_manifest, qualify as qualify_causal_integrity, CausalEdge4,
    CausalIntegrityArtifact4, CausalIntegrityCard7, CausalIntegrityError, CausalIntegrityRequest4,
    BOUNDARY as CAUSAL_INTEGRITY_BOUNDARY, CONTENT_TYPE as CAUSAL_INTEGRITY_CONTENT_TYPE,
};
pub use error::WorldError;
pub use event::CausalEvent;
pub use fact::Fact;
pub use factor::Factor;
pub use federated_continual_causal_integrity_contract_model::*;
pub use federated_continual_causal_integrity_inference::*;
pub use federated_continual_causal_integrity_research_copilot::*;
pub use federated_continual_causal_integrity_workflow_fabric::*;
pub use index::WorldIndex;
pub use local_causal_integrity_contract_model::*;
pub use local_causal_integrity_inference::*;
pub use local_causal_integrity_research_copilot::*;
pub use local_causal_integrity_workflow_fabric::*;
pub use multimodal_causal_integrity_contract_model::*;
pub use multimodal_causal_integrity_inference::*;
pub use multimodal_causal_integrity_research_copilot::*;
pub use multimodal_causal_integrity_workflow_fabric::*;
pub use source::WorldSource;
pub use throughput_causal_integrity_contract_model::*;
pub use throughput_causal_integrity_inference::*;
pub use throughput_causal_integrity_research_copilot::*;
pub use throughput_causal_integrity_workflow_fabric::*;
pub use validate::{validate, Diagnostic, Severity, ValidationReport};
pub use world::{World, WORLD_SCHEMA_VERSION};
