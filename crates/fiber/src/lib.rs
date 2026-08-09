//! The FIBER query compiler.
//!
//! Implements blueprint 43.13 (protected closure), 43.17 (dependency slicing and obligation
//! closure), 43.09 (temporal accessibility), 43.16 (the compiler pipeline), the fragment of 43.33
//! (policy fibers) the v0.1 wire formats can state, and the deterministic oracle of 43.41,
//! emitting the Decision Section and Context Certificate defined in `bioprism-section`.
//!
//! ```no_run
//! use bioprism_fiber::{compile, Query};
//! use bioprism_world::World;
//!
//! let world = World::from_json(serde_json::from_str(&std::fs::read_to_string("world.json")?)?)?;
//! let query = Query::from_json(serde_json::from_str(&std::fs::read_to_string("query.json")?)?)?;
//! let out = compile(&world, &query)?;
//! println!("{}", out.certificate.digest(bioprism_section::CertificateProfile::Reference)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! What this engine does *not* do is as important as what it does. It performs no gluing or
//! obstruction analysis, no abstract interpretation, no decision-equivalence quotienting and no
//! rate-distortion optimisation, because `fiber-world/0.1` and `fiber-query/0.2` do not carry the
//! cover, abstract domains, permitted actions or decision loss those passes are defined against.
//! Every compile reports the gap in [`CompileTrace::deferred_passes`] and on the certificate's
//! `limitations`.
//!
//! The [`policy`] pass is the same story told at one level of detail rather than none: it enforces
//! the clause grants the wire formats *can* express and names, in its module documentation, the
//! six 43.33 mechanisms they cannot. `bioprism-policy` is where the full fiber lives, and this
//! crate deliberately does not depend on it.

pub mod closure;
pub mod compile;
pub mod error;
pub mod oracle;
pub mod policy;
pub mod qir;
pub mod slice;
pub mod temporal;

pub use compile::{compile, CompileOutput, CompileTrace, PassReceipt};
pub use error::FiberError;
pub use policy::{PolicyEnvelope, PolicyOutcome, PolicyScreen, PolicyViolation};
pub use qir::{
    Budgets, Query, ACCEPTED_QUERY_SCHEMA_VERSIONS, QUERY_FIELD_PATHS, QUERY_SCHEMA_VERSION,
    REFERENCE_GOAL,
};
pub use slice::{backward_slice, Slice};
pub use temporal::{temporal_cut, TemporalCut};
