//! Typed failures for slice construction and execution.
//!
//! Blueprint 40.36 requires a documented error taxonomy. The distinction this taxonomy exists to
//! preserve is between *the slice broke* and *the platform's claim stopped holding*. A world that
//! will not load is the former: the example itself is defective and no conclusion about FIBER may
//! be drawn from it. A compile whose verdict no longer matches the slice's declared expectation is
//! the latter, and that is deliberately **not** an error — it is a recorded failure on
//! [`crate::SliceReport`], because a claim that stopped holding is a result, not a crash.

use thiserror::Error;

/// Something went wrong building or running a reference example.
#[derive(Debug, Error)]
pub enum ExampleError {
    /// The generated world document did not load. The example is broken, not the platform.
    #[error("slice {slice:?}: generated world does not load: {source}")]
    World {
        slice: String,
        #[source]
        source: bioprism_world::WorldError,
    },

    /// The generated query document did not parse.
    #[error("slice {slice:?}: generated query does not parse: {source}")]
    Query {
        slice: String,
        #[source]
        source: bioprism_fiber::FiberError,
    },

    /// A report could not be canonicalised, so it cannot be digested and cannot be replayed.
    #[error("slice {slice:?}: report cannot be canonically encoded: {source}")]
    Digest {
        slice: String,
        #[source]
        source: bioprism_ids::CanonicalError,
    },

    /// A caller asked the registry for a slice that is not registered.
    #[error("no slice registered under id {0:?}")]
    UnknownSlice(String),

    /// Two registered slices share an id, so reports cannot be attributed unambiguously.
    #[error("duplicate slice id {0:?}: slice reports would be ambiguous")]
    DuplicateSlice(String),
}
