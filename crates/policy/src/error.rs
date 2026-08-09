//! Typed construction and registration failures.
//!
//! Separate from [`crate::decision::Refusal`], and the separation is deliberate. A `Refusal` is a
//! correct answer: the policy held and the request did not qualify. A [`PolicyError`] means the
//! policy itself is malformed — a rule registered twice with different content, a declassification
//! that declassifies nothing, a purpose name nobody defined. Collapsing the two would let a
//! broken rule set present as a stream of ordinary denials, which is the quiet failure mode that
//! 36.01's release gate ("an independent reviewer verifies the policy implementation") exists to
//! catch.
//!
//! [`PolicyError::Refused`] exists so a caller that wants `?` semantics can lift a refusal into
//! the error channel at its own boundary, not because this crate does so internally.

use crate::decision::Refusal;
use bioprism_scope::ScopeError;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PolicyError {
    #[error("unknown purpose {0:?}: purposes are a closed set and cannot be minted by spelling")]
    UnknownPurpose(String),

    #[error("unknown classification {0:?}")]
    UnknownClassification(String),

    #[error("policy rule {id:?} is registered twice at version {version} with different content")]
    RuleVersionConflict { id: String, version: u32 },

    #[error(
        "declassification rule {id:?} v{version} carries no leakage analysis; 43.33 requires \
         one before an approved operator may lower a label"
    )]
    UnanalysedDeclassification { id: String, version: u32 },

    #[error(
        "declassification rule {id:?} v{version} does not lower its source label, so it is a \
         relabelling rather than a declassification"
    )]
    NotADeclassification { id: String, version: u32 },

    #[error("redaction rule {id:?} v{version} states no rationale, so its receipts would be blank")]
    UnexplainedRedaction { id: String, version: u32 },

    #[error("the policy rule set could not be canonicalised for versioning: {0}")]
    Uncanonicalisable(String),

    #[error(transparent)]
    Refused(#[from] Refusal),

    #[error(transparent)]
    Scope(#[from] ScopeError),
}
