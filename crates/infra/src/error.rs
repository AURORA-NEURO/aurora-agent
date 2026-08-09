//! Typed failures, one enum per concern.
//!
//! Blueprint 12.10's mitigation for every listed failure is the same sentence: "detect the
//! condition explicitly, fail closed where integrity or safety is affected, preserve the
//! underlying evidence, and emit an actionable diagnostic rather than silently repairing or
//! discarding state." That sentence is repeated verbatim in all 22 modules of section 12, which
//! makes it boilerplate, but it is also the right rule, so the variants below carry the evidence
//! rather than a message: which component of a key differed, which resources are unknown, which
//! manifest still references the object being deleted.
//!
//! Every enum is `Clone + PartialEq` so tests assert on the exact failure rather than on a
//! substring of its `Display`, which is how an error taxonomy rots into prose.

use crate::epoch::Epoch;
use bioprism_ids::CanonicalError;
use thiserror::Error;

/// Failures of the cache: key construction, schema agreement, and the one alarming case.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CacheError {
    /// A key omitted a component the schema declares. The component names a thing that
    /// determines the result, so a key without it addresses a computation nobody specified.
    #[error("key for schema {schema:?} is missing required component {component:?}")]
    IncompleteKey { schema: String, component: String },

    /// A component was present but empty. Indistinguishable from absence at digest time, and a
    /// far more common way to lose the environment from a key than forgetting the field.
    #[error("component {component:?} of schema {schema:?} is empty")]
    EmptyComponent { schema: String, component: String },

    /// A key carried a component the schema does not declare. Refused rather than ignored: if
    /// two callers disagree about the component set, they disagree about what determines the
    /// result, and one of their cache entries is wrong.
    #[error("component {component:?} is not declared by schema {schema:?}")]
    UndeclaredComponent { schema: String, component: String },

    /// A schema declaring no components would make every computation share one address.
    #[error("schema {schema:?} declares no components")]
    SchemaWithoutComponents { schema: String },

    /// A schema, component or build identifier was blank or held control characters.
    #[error("malformed {field}: {value:?}")]
    MalformedField { field: &'static str, value: String },

    /// The key presented belongs to a different schema than the cache was built for.
    #[error("cache holds schema {expected:?} but the key names schema {presented:?}")]
    ForeignSchema { expected: String, presented: String },

    /// Two different computations reduced to one digest.
    ///
    /// The reason this crate exists. A digest match is a *candidate*, and this variant is what a
    /// candidate that fails the component-by-component check becomes. Reachable in practice
    /// through [`crate::cache::Cache::restore`] of an index written by a build with a different
    /// canonicalization; reachable in theory through a SHA-256 collision. Either way it is never
    /// served.
    #[error("digest {digest} maps to two computations: component {component:?} is {stored:?} in the cache but {presented:?} in the request")]
    KeyCollision {
        digest: String,
        component: String,
        stored: String,
        presented: String,
    },

    /// The key could not be canonically serialized, so it has no stable address.
    #[error("key cannot be canonically serialized: {0}")]
    NotCanonical(String),
}

impl From<CanonicalError> for CacheError {
    fn from(error: CanonicalError) -> Self {
        CacheError::NotCanonical(error.to_string())
    }
}

/// Failures of the dependency graph and the invalidation walk over it.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InvalidationError {
    #[error("malformed resource id: {0:?}")]
    MalformedResource(String),

    /// A resource cannot both declare its dependencies and be opaque; the graph would then be
    /// simultaneously trusted and untrusted for the same node.
    #[error("resource {0:?} is declared both with dependencies and as opaque")]
    ContradictoryDeclaration(String),

    /// Applying a plan computed against a different entry population would remove entries the
    /// plan never examined, or miss entries it should have marked.
    #[error(
        "invalidation plan was computed over {planned} entries but the cache now holds {actual}"
    )]
    PopulationChanged { planned: usize, actual: usize },
}

/// Failures of the data-quality gates.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QualityError {
    #[error("column {column:?} has {found} values but the dataset has {expected} rows")]
    RaggedColumn {
        column: String,
        found: usize,
        expected: usize,
    },

    #[error("column {0:?} is declared twice")]
    DuplicateColumn(String),

    /// Two checks under one name make the report's outcome map lossy, and a silently dropped
    /// check is a gate that passes for the wrong reason.
    #[error("check {0:?} is declared twice in this gate")]
    DuplicateCheckName(String),

    #[error("malformed {field}: {value:?}")]
    MalformedField { field: &'static str, value: String },
}

/// Failures of tiering, lifecycle and reachability collection.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    #[error("malformed {field}: {value:?}")]
    MalformedField { field: &'static str, value: String },

    /// Cold must be reached later than warm, or the plan would demote straight past a tier the
    /// policy claims to use.
    #[error("tiering policy demotes to cold after {cold} idle epochs but to warm after {warm}")]
    IncoherentTieringPolicy { warm: u64, cold: u64 },

    /// An object was last accessed after the epoch being planned for.
    #[error("object {object:?} was last accessed at {last_access} which is after the planning epoch {now}")]
    AccessInFuture {
        object: String,
        last_access: Epoch,
        now: Epoch,
    },

    #[error("no object {0:?} under management")]
    UnknownObject(String),

    /// The object is still named by a manifest. Deleting it would leave a published result
    /// pointing at nothing, which 40.06 forbids by requiring deletion to leave a tombstone
    /// rather than rewrite result history.
    #[error("object {object:?} is still referenced by {by:?}")]
    StillReferenced { object: String, by: String },

    /// Reclamation reached inside the window the ledger still promises to answer.
    ///
    /// Storage may not reclaim ahead of the ledger: if `bioprism-ledger` will still answer an
    /// as-of query about an event, the object that event names must still resolve. Lawful
    /// deletion under 12.22 is the exception and takes a different path.
    #[error("object {object:?} was created at {created} which is inside the retained window beginning at {retained_from}; reclamation would strand a ledger answer")]
    WithinRetainedWindow {
        object: String,
        created: String,
        retained_from: String,
    },

    /// The ledger has never compacted, so it still promises to answer about everything and no
    /// object is behind a boundary. Reported separately from
    /// [`LifecycleError::WithinRetainedWindow`] because the fix is different: compact the ledger
    /// first, rather than wait.
    #[error("object {object:?} cannot be reclaimed: the ledger retention window is unrestricted, so every object is still answerable")]
    RetentionWindowUnrestricted { object: String },

    /// The object was deleted; the tombstone is what remains.
    #[error("object {object:?} was deleted at {at}: {reason}")]
    Tombstoned {
        object: String,
        at: Epoch,
        reason: String,
    },
}

/// Failures of the storage budget.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QuotaError {
    /// The write would cross the hard limit. 12.21: "hard budget stops safe work".
    #[error("writing {requested} bytes to {class} exceeds the quota: {available} of {limit} bytes available")]
    Exceeded {
        class: &'static str,
        requested: u64,
        available: u64,
        limit: u64,
    },

    /// The write would cross into the reserve, which only cleanup and evidence finalization may
    /// draw on. 12.21: "reserve protects cleanup/evidence finalization".
    #[error("writing {requested} bytes for purpose {purpose} would draw on the {reserve}-byte reserve, which only cleanup and evidence finalization may use")]
    ReserveIsProtected {
        purpose: &'static str,
        requested: u64,
        reserve: u64,
    },

    /// Releasing more than was charged would mint allowance out of nothing.
    #[error("releasing {requested} bytes from {class} but only {charged} are charged")]
    ReleaseExceedsCharge {
        class: &'static str,
        requested: u64,
        charged: u64,
    },

    /// A delegation asked for more than the parent holds.
    #[error("cannot delegate {requested} bytes: only {available} are unspent")]
    DelegationExceedsRemaining { requested: u64, available: u64 },

    /// A reserve larger than the limit leaves no ordinary allowance at all.
    #[error("reserve of {reserve} bytes is not smaller than the limit of {limit}")]
    ReserveExceedsLimit { reserve: u64, limit: u64 },
}

/// Failures of the rebuildable projections.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IndexError {
    #[error("malformed {field}: {value:?}")]
    MalformedField { field: &'static str, value: String },

    /// A rebuild claimed to cover a range ending before the one already covered, which would
    /// make the projection's freshness go backwards while its revision went forwards.
    #[error("rebuild of {index:?} covers through {through} but the existing revision already covers through {existing}")]
    RebuildGoesBackwards {
        index: String,
        through: Epoch,
        existing: Epoch,
    },
}

/// Failures of backup and restore.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BackupError {
    #[error("malformed {field}: {value:?}")]
    MalformedField { field: &'static str, value: String },

    /// A restore was asked to run classes out of the order 12.19 requires: trust roots and
    /// metadata first, then artifacts, then rebuildable projections.
    #[error("restore order places {later} before {earlier}, but {earlier} must be restored first")]
    OutOfOrder {
        earlier: &'static str,
        later: &'static str,
    },
}
