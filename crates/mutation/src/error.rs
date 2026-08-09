//! The crate's failure taxonomy.
//!
//! Blueprint 40.36's first invariant is a typed error at every service boundary, and 40.22 names
//! [`crate::lineage::generate`] as this crate's boundary: it is what a caller building a benchmark
//! family actually invokes. A `String` there erased three failures with three different remedies
//! into one opaque sentence — a world that does not load is the caller's document to fix, a world
//! the oracle cannot judge is the oracle's contract to widen, and a world that cannot be
//! canonically serialised is a number that should never have been written.
//!
//! Every variant names the world it is about. The strings these replaced did not, so a caller that
//! saw `duplicate fact id: fact.split` could not tell whether the parent it supplied was at fault
//! or a descendant one of its own mutations had produced.
//!
//! [`RejectionReason`] is here for the same reason and is deliberately *not* the same thing. A
//! [`MutationError`] stops generation; a rejection is recorded and generation continues, because a
//! generator that drops its failures reports an impressive yield rate and an unfalsifiable one.

use crate::apply::ApplyError;
use bioprism_fiber::FiberError;
use bioprism_ids::CanonicalError;
use bioprism_world::WorldError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A failure that stops generation.
///
/// Each variant carries the underlying typed error as its `source` rather than its rendering, so a
/// caller that wants to branch on, say, [`FiberError::UnorderableSplitGroups`] still can.
#[derive(Debug, Error, PartialEq, Clone)]
pub enum MutationError {
    /// The world cannot be canonically serialised, so it has no content digest.
    ///
    /// Fatal rather than recorded: deduplication is by content digest, so admitting an
    /// undigestable world would mean a family that had quietly stopped deduplicating while still
    /// reporting a duplicate count of zero.
    #[error("world {world_id:?} cannot be canonically serialised, so it has no content digest and cannot be deduplicated: {source}")]
    NotAddressable {
        world_id: String,
        #[source]
        source: CanonicalError,
    },

    /// The world document did not pass the world loader's acceptance checks.
    #[error("world {world_id:?} does not load: {source}")]
    DoesNotLoad {
        world_id: String,
        #[source]
        source: WorldError,
    },

    /// The world loaded, and the oracle refused to return a verdict over its facts.
    ///
    /// Distinct from [`MutationError::DoesNotLoad`] because the remedy is different: the document
    /// is well formed and the oracle's own preconditions are what failed.
    #[error("the oracle could not evaluate world {world_id:?}: {source}")]
    NotEvaluable {
        world_id: String,
        #[source]
        source: FiberError,
    },
}

/// Why a mutation was not admitted to a family.
///
/// Recorded rather than returned. The five variants are the five places a mutation can fall out of
/// [`crate::lineage::generate`], and they are kept apart because they are findings about different
/// code: `ParentMalformed` indicts the world the caller supplied, `NotApplicable` indicts the
/// pairing of mutation and world, `DescendantDoesNotLoad` and `DescendantNotEvaluable` indict the
/// transformation, and `PostconditionViolated` indicts the relation the mutation declared. A
/// caller reading a family should not have to parse prose to tell them apart.
///
/// The `detail` fields carry the rendering of an already-typed underlying error. The variant is
/// the classification; the detail is the provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(tag = "rejection", rename_all = "snake_case")]
pub enum RejectionReason {
    /// The parent world is missing structure every mutation needs.
    #[error("the parent world is malformed: {detail}")]
    ParentMalformed { detail: String },

    /// The parent world is well formed and this mutation has nothing to act on in it.
    #[error("the mutation is not applicable to this world: {detail}")]
    NotApplicable { detail: String },

    /// The transformation produced a document the world loader rejects.
    #[error("the mutated world does not load: {detail}")]
    DescendantDoesNotLoad { detail: String },

    /// The transformation produced a loadable world the oracle refused to judge.
    #[error("the oracle could not evaluate the mutated world: {detail}")]
    DescendantNotEvaluable { detail: String },

    /// The oracle ran and disagreed with what the mutation declared about it.
    ///
    /// The only rejection that is a defect in the *mutation* rather than in its inputs, and the
    /// one the metamorphic engine exists to catch.
    #[error("postcondition violated: expected {expected}, observed {observed}")]
    PostconditionViolated { expected: String, observed: String },
}

impl RejectionReason {
    /// Projects an [`ApplyError`] onto the rejection taxonomy.
    ///
    /// Matched variant by variant rather than through a wildcard, so a new way for a
    /// transformation to refuse forces a decision here instead of silently joining an existing
    /// class.
    pub(crate) fn from_apply(error: &ApplyError) -> Self {
        match error {
            ApplyError::Malformed(detail) => RejectionReason::ParentMalformed {
                detail: (*detail).to_string(),
            },
            ApplyError::NotApplicable(detail) => RejectionReason::NotApplicable {
                detail: detail.clone(),
            },
        }
    }
}
