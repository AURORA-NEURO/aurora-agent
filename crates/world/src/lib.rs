//! The FIBER world model.
//!
//! Implements the data half of blueprint 43.02 (formal object), 43.04 (evidence fibration and
//! local sections), 43.07 (factorized evidence algebra) and 43.09 (causal event structures),
//! against the `fiber-world/0.1` wire schema.
//!
//! This crate is deliberately inert: it parses, indexes and diagnoses. Every decision about
//! *which* of these objects a query needs belongs to the compiler, so that the world can be
//! shared unchanged across queries, roles and policies.

pub mod error;
pub mod event;
pub mod fact;
pub mod factor;
pub mod index;
pub mod source;
pub(crate) mod json;
pub mod validate;
pub mod world;

pub use error::WorldError;
pub use event::CausalEvent;
pub use fact::Fact;
pub use factor::Factor;
pub use index::WorldIndex;
pub use source::WorldSource;
pub use validate::{validate, Diagnostic, Severity, ValidationReport};
pub use world::{World, WORLD_SCHEMA_VERSION};
