//! Typed failures for schema evolution.
//!
//! Blueprint 40.37 lists four failure modes — non-deterministic migration, semantic loss, partial
//! DB migration, old client misreads new object — and requires each to surface as a typed event
//! rather than a log line. Three of the four are representable here. *Partial DB migration* is not:
//! this crate has no database and no transaction, so it cannot half-apply anything.
//!
//! Errors carry the observation that produced them, not just the rule that was violated. "Schema
//! mismatch" is a verdict; "reader declares reject, writer declares preserve-and-forward" is a work
//! item.

use crate::descriptor::DigestRole;
use crate::mode::CompatibilityMode;
use crate::version::VersionBump;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A malformed version or schema identifier.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum VersionError {
    #[error("schema identifier {0:?} has no '/' separating name from version")]
    MissingSeparator(String),

    #[error("schema identifier has an empty name: {0:?}")]
    EmptyName(String),

    #[error("version {value:?} is malformed: {detail}")]
    Malformed { value: String, detail: String },
}

/// A schema descriptor that does not describe anything checkable.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum DescriptorError {
    #[error("field path is empty")]
    EmptyPath,

    #[error("field path {0:?} has an empty segment")]
    EmptySegment(String),

    #[error("field {0:?} is declared twice")]
    DuplicateField(String),

    #[error("field {path:?} is declared under {parent:?}, which is an opaque object")]
    DescendsIntoOpaqueField { path: String, parent: String },
}

/// A reader and a writer that cannot safely exchange documents.
///
/// 43.35: *unknown fields are handled according to declared compatibility mode*. The mode is a
/// declaration on both sides, so disagreement is detectable before a byte moves, and
/// [`CompatibilityError::ModeDisagreement`] is that detection.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CompatibilityError {
    #[error(
        "writer declares {writer} and reader declares {reader}; 43.35 requires a declared mode, \
         and two declarations that disagree are a negotiation failure, not a default"
    )]
    ModeDisagreement {
        writer: CompatibilityMode,
        reader: CompatibilityMode,
    },

    #[error("reader rejects {count} unknown field(s) under reject mode, first: {first:?}")]
    UnknownFieldsRejected { count: usize, first: String },

    #[error(
        "{name}: {from} -> {to} is classified {class} and requires a {required} bump, but the \
         observed bump is {observed}"
    )]
    VersionBumpTooSmall {
        name: String,
        from: String,
        to: String,
        class: String,
        required: VersionBump,
        observed: VersionBump,
    },

    #[error("cannot diff {from} against {to}: different schema names are not a version lineage")]
    DifferentSchemas { from: String, to: String },

    #[error(
        "cannot diff {from} against {to}: same release, different variant label; a labelled \
         variant is a sibling of its base, not a successor"
    )]
    NotASuccessor { from: String, to: String },
}

/// A migration that is not checkable, not total, or not honest about what it drops.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum MigrationError {
    #[error("migration declares {from} -> {to}, but the document declares schema {found:?}")]
    SourceVersionMismatch {
        from: String,
        to: String,
        found: String,
    },

    #[error("migration {from} -> {to} would be a no-op between identical versions")]
    IdentityMigration { from: String, to: String },

    #[error("step {step} cannot apply: {detail}")]
    StepFailed { step: usize, detail: String },

    #[error("document is not a JSON object, so it has no fields to migrate")]
    NotAnObject,

    #[error("no registered migration path from {from} to {to}")]
    NoPath { from: String, to: String },

    #[error("a migration from {from} to {to} is already registered")]
    DuplicateMigration { from: String, to: String },

    #[error("step {step} is not invertible: {detail}")]
    NotInvertible { step: usize, detail: String },

    #[error(
        "migration {from} -> {to} lost {path:?} without declaring it; 40.37 invariant 3 is that a \
         lossy migration is never silent"
    )]
    UndeclaredLoss {
        from: String,
        to: String,
        path: String,
    },
}

/// A lifecycle transition that skips a stage, moves backwards, or omits its justification.
///
/// There is no clock in this crate. Every epoch in these errors was supplied by an operator, so a
/// dwell violation means the operator asked to advance too early by their own accounting, not that
/// wall-clock time was consulted.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum DeprecationError {
    #[error(
        "{subject:?} cannot move {from} -> {to}: the lifecycle advances one stage at a time, so a \
         field cannot go straight to removed"
    )]
    StageSkipped {
        subject: String,
        from: String,
        to: String,
    },

    #[error("{subject:?} cannot move {from} -> {to}: the lifecycle does not run backwards")]
    StageReversed {
        subject: String,
        from: String,
        to: String,
    },

    #[error("{subject:?} transition to {to} states no reason")]
    ReasonMissing { subject: String, to: String },

    #[error(
        "{subject:?} transition to {to} names no replacement and gives no justification for \
         having none"
    )]
    ReplacementMissing { subject: String, to: String },

    #[error("{subject:?} transition to {to} is at epoch {epoch}, not after epoch {previous}")]
    EpochNotAdvancing {
        subject: String,
        to: String,
        epoch: u64,
        previous: u64,
    },

    #[error(
        "{subject:?} spent {observed} epoch(s) in {stage}, policy requires {required}; an \
         expedited transition may waive the dwell but must carry an advisory"
    )]
    DwellTooShort {
        subject: String,
        stage: String,
        observed: u64,
        required: u64,
    },

    #[error("{subject:?} expedited transition to {to} carries no advisory")]
    AdvisoryMissing { subject: String, to: String },

    #[error("{0:?} is already recorded in this ledger")]
    DuplicateSubject(String),

    #[error(
        "schema change removes {path:?}, which is {stage} rather than removed; a field leaves the \
         wire format only after its lifecycle has finished"
    )]
    RemovedBeforeSunset { path: String, stage: String },

    #[error("schema change removes {path:?}, which has no deprecation record at all")]
    RemovedWithoutRecord { path: String },
}

/// A schema change that would move an existing artifact's digest.
///
/// This is the failure the rest of the workspace cannot absorb. Three implementations currently
/// agree on the reference certificate digest; a change in this class silently ends that agreement,
/// which is why it is a distinct error rather than a classification result.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[error(
    "{path:?} ({role}) would alter the digest of every existing {schema} artifact: {detail}. This \
     cannot be classified compatible at any version."
)]
pub struct DigestBreach {
    pub schema: String,
    pub path: String,
    pub role: DigestRole,
    pub detail: String,
}

/// The crate's outward error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GovernanceError {
    #[error(transparent)]
    Version(#[from] VersionError),

    #[error(transparent)]
    Descriptor(#[from] DescriptorError),

    #[error(transparent)]
    Compatibility(#[from] CompatibilityError),

    #[error(transparent)]
    Migration(#[from] MigrationError),

    #[error(transparent)]
    Deprecation(#[from] DeprecationError),

    #[error(transparent)]
    Digest(#[from] DigestBreach),

    #[error("canonical serialisation failed: {0}")]
    Canonical(String),
}

impl From<bioprism_ids::CanonicalError> for GovernanceError {
    fn from(error: bioprism_ids::CanonicalError) -> Self {
        GovernanceError::Canonical(error.to_string())
    }
}
