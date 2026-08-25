//! Canonical serialization, content hashing and typed identifiers.
//!
//! This is the bottom of the BioPRISM dependency graph: it depends on no other workspace
//! crate, and every crate that emits a hash, a certificate or an identifier depends on it.
//!
//! Implements blueprint 40.05 (canonical identifiers and hashes) and supplies the hashing
//! primitive that 43.26 (Context Certificate) requires to be replayable across languages.

pub mod canonical;
pub mod error;
pub mod evolution;
pub mod hash;
pub mod id;

pub use canonical::{python_repr_f64, to_canonical_bytes, to_canonical_string};
pub use error::{CanonicalError, IdError};
pub use evolution::{
    EvolutionIdentity, EvolutionIdentityError,
    CONTRACT_VERSION as EVOLUTION_IDENTITY_CONTRACT_VERSION,
    FEATURE_ID as EVOLUTION_IDENTITY_FEATURE_ID,
    PRECLINICAL_BOUNDARY as EVOLUTION_IDENTITY_BOUNDARY,
};
pub use hash::{sha256_hex_of_value, ContentHash};
pub use id::{EventId, FactId, FactorId, QueryId, RunId, VariableName, WorldId};
