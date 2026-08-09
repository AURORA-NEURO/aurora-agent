//! Typed failures for world construction and characterisation.
//!
//! Blueprint 43.06 requires structured failure rather than a boolean. A world that will not build
//! is not a bug report; it is a statement about which structural invariant the caller's spec
//! violated, and that statement has to survive being serialised into a slice report.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BioWorldError {
    /// The document does not load under `fiber-world/0.1`.
    ///
    /// Carried as a string rather than as `bioprism_world::WorldError` because a slice report is
    /// `Serialize` and `WorldError` is not; the alternative — dropping the reason — is worse.
    #[error("world {world_id} does not load under fiber-world/0.1: {message}")]
    WorldRejected { world_id: String, message: String },

    /// A timestamp literal in a world or query shape is not RFC3339.
    #[error("{subject} carries an unparseable timestamp {value:?}: {message}")]
    Timestamp {
        subject: String,
        value: String,
        message: String,
    },

    /// A slice names a target the world does not produce.
    #[error("slice {slice} targets {target:?}, which no factor in world {world_id} outputs")]
    UnknownTarget {
        slice: String,
        target: String,
        world_id: String,
    },

    /// A slice names a variable no fact provides and no factor produces.
    #[error("slice {slice} refers to variable {variable:?}, which world {world_id} does not define")]
    UnknownVariable {
        slice: String,
        variable: String,
        world_id: String,
    },

    /// Two slices share an id, so a report could not be attributed to one of them.
    #[error("duplicate slice id {0}")]
    DuplicateSlice(String),

    #[error("no slice with id {0}")]
    UnknownSlice(String),

    /// A digest could not be taken over a report body.
    #[error("could not digest {subject}: {message}")]
    Digest { subject: String, message: String },

    /// A borrowed vocabulary term did not serialise to a bare string.
    ///
    /// Variable names in [`crate::underdetermined`] are derived from `bioprism-onco`'s serde
    /// representation rather than retyped, so the crate's vocabulary stays the single source. That
    /// works for unit variants only; a data-carrying variant serialises to an object and is
    /// refused here rather than stringified into an invented name.
    #[error("{subject} does not serialise to a bare string, so no variable name can be derived from it")]
    VocabularyNotNameable { subject: String },
}
