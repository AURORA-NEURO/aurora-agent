//! The typed scope base.
//!
//! Implements blueprint 43.03 (typed scope base), the mapping taxonomy of 43.05
//! (restriction, transport, base change) and the structured-failure requirement of 43.06.
//!
//! Evidence in BioPRISM is valid only inside a scope. This crate supplies the type that says
//! *where* a claim holds, the partial order that says when one scope is narrower than another,
//! and the meet that says whether two scopes overlap at all — plus the vocabulary for moving
//! evidence between scopes without pretending the move was free.

pub mod class;
pub mod error;
pub mod key;
pub mod meet;
pub mod time;
pub mod transport;

pub use class::{DimensionRegistry, ScopeClass};
pub use error::{ScopeError, TimeError};
pub use key::{ScopeKey, ScopeValue};
pub use meet::{meet, EmptyReason, Meet};
pub use time::{Interval, Timestamp};
pub use transport::{
    AggregationOperator, LossLedger, MappingCheck, MappingKind, ScopeMapping,
};
