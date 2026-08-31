//! Why a figure was not rendered.
//!
//! Every variant names exactly what the caller must fix. The rule these variants exist to enforce
//! is the crate's parsing contract: a field the figure needs but cannot find is an error naming
//! that field, never a silent zero, and a document that contradicts itself is refused rather than
//! rendered as if it were coherent.

/// The single error type every figure function returns.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FigureError {
    /// A field the figure renders is absent. The path is dotted from the input root, with array
    /// indices in brackets (`results[3].facts_exposed`), so the caller can find it without
    /// re-reading this crate.
    #[error("input is missing required field `{field}`")]
    MissingField { field: String },

    /// The field exists but holds the wrong JSON type. Distinct from [`FigureError::MissingField`]
    /// because the two defects have different fixes and coercing one into the other would hide
    /// which document producer is broken.
    #[error("field `{field}` must be {expected}")]
    WrongType { field: String, expected: &'static str },

    /// A collection the figure draws from holds no entries, so there is nothing to draw. An empty
    /// panel is not a figure of zero bars.
    #[error("`{field}` is empty, so there is nothing to draw")]
    EmptyCollection { field: String },

    /// The document contradicts itself — for example an `admissible` flag disagreeing with the
    /// verdict fields it is defined from, or a compiled count exceeding its total. Rendering such
    /// a document would lend it a coherence it does not have.
    #[error("input is internally inconsistent: {reason}")]
    Inconsistent { reason: String },

    /// The input could not be canonically digested for the `source sha256` footer. Without the
    /// digest the figure cannot state what it is a figure of, so nothing is rendered.
    #[error("input could not be canonically digested for the source footer: {reason}")]
    Canonicalisation { reason: String },
}
