//! The typed failures of the developer platform itself.
//!
//! These are the errors this crate raises when a developer builds a *diagnostic* wrongly, or asks
//! the introspection record a question it cannot answer. They are deliberately distinct from
//! [`crate::Diagnostic`], which is what this crate produces *about* somebody else's compile: a
//! diagnostic is a datum, an error here is a refusal.
//!
//! The distinction matters because the two are easy to conflate. If malformed diagnostics were
//! themselves reported as diagnostics, the catalogue lint would grade its own bug reports and the
//! crate would have no failure mode left that a caller must handle.
//!
//! Every variant names both sides of the disagreement wherever there are two, matching the house
//! rule set by `bioprism-sdk`'s error module.

use thiserror::Error;

/// A diagnostic code that does not satisfy the code grammar.
///
/// Codes are the stable join key between a catalogue entry, a lint finding and an exit-code audit
/// row. A code that varies by build is worse than no code, so the grammar is checked at
/// construction rather than trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CodeError {
    #[error("a diagnostic code cannot be empty")]
    Empty,

    #[error(
        "diagnostic code {code:?} is not in the DEVX-NNNN form; a code that varies in shape \
         cannot be matched by a consumer"
    )]
    Malformed { code: String },

    #[error("diagnostic code {code:?} is outside the DEVX-0001..DEVX-9999 range")]
    OutOfRange { code: String },
}

/// A catalogue that cannot be trusted as a lookup table.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogueError {
    #[error(transparent)]
    Code(#[from] CodeError),

    #[error(
        "diagnostic code {code} is registered twice, by {incumbent:?} and {challenger:?}; a code \
         that names two rules cannot be looked up"
    )]
    DuplicateCode {
        code: String,
        incumbent: String,
        challenger: String,
    },

    #[error("no diagnostic is registered under code {code}")]
    UnknownCode { code: String },
}

/// A question the introspection record was not built to answer.
///
/// `NotRecorded` is not in here on purpose. "The record does not say" is an *answer* — see
/// [`OmissionAnswer::NotRecorded`](crate::introspect::OmissionAnswer::NotRecorded) — and demoting
/// it to an error would let a caller `unwrap_or_default()` it into silence, which is exactly the
/// failure `bioprism-section`'s omission manifest exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IntrospectError {
    #[error("the compile record names no pass called {pass:?}")]
    UnknownPass { pass: String },

    #[error(
        "pass {pass:?} appears {count} times in the record; a pass identity that repeats cannot \
         carry a decision"
    )]
    DuplicatePass { pass: String, count: usize },

    #[error("the compile record carries no certificate digest, so {question} cannot be answered")]
    NoCertificate { question: String },
}

/// A local-loop contract that does not describe a usable surface.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LoopError {
    #[error("surface {surface:?} is declared twice in the loop contract")]
    DuplicateSurface { surface: String },

    #[error(
        "surface {surface:?} declares no invalidation at all; a surface that invalidates nothing \
         is either mis-scoped or should not be declared"
    )]
    EmptyInvalidation { surface: String },

    #[error("no surface in the loop contract owns {subject:?}, so its blast radius is unknown")]
    UnownedSubject { subject: String },
}

/// The crate-level error, for callers that do not want to match on five enums.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DevxError {
    #[error(transparent)]
    Code(#[from] CodeError),

    #[error(transparent)]
    Catalogue(#[from] CatalogueError),

    #[error(transparent)]
    Introspect(#[from] IntrospectError),

    #[error(transparent)]
    Loop(#[from] LoopError),
}
