//! Schema evolution and the compatibility contract.
//!
//! Implements blueprint 40.37 (Schema, Database and Artifact Migrations), 25.22 (Schema
//! Versioning, Compatibility and Migration), the versioning and deprecation policy of section 14 —
//! chiefly 14.16 and 14.17 — and the clause of 43.35 that everything else here hangs off:
//! *unknown fields are handled according to declared compatibility mode*.
//!
//! # The one guarantee this crate exists to protect
//!
//! Three implementations currently agree on the reference Context Certificate digest: CPython, the
//! eager Rust path and the indexed store. That agreement is not a nice property, it is the
//! platform's claim to being verifiable by someone who does not trust the publisher. A schema
//! change is the cheapest way to end it silently — add a field with a default, drop a field an old
//! reader does not recognise, move a field across the boundary of the hashed bytes, and every
//! previously published artifact either fails to verify or verifies to something new.
//!
//! So [`classify::affects_digest`] is a predicate over a single field change, any change for which
//! it holds is [`classify::CompatibilityClass::Breaking`] regardless of how it looks field by
//! field, and [`classify::Classification::assert_class`] refuses an author's claim of
//! compatibility with a typed [`error::DigestBreach`]. Everything else in this crate is scaffolding
//! for that rule.
//!
//! # Earned, not asserted
//!
//! Nothing here takes the author's word for anything.
//!
//! | Question | How it is answered |
//! |---|---|
//! | Is this change compatible? | [`diff::diff`] computes the field-level change list; the class is the maximum severity over it. An author who labels a breaking change minor is contradicted by [`classify::Classification::audit_claim`], with the offending changes as evidence. |
//! | Did the version move far enough? | [`classify::Classification::version_gate`] compares the bump required by the derived class against the bump read off the two version strings. |
//! | Is this migration total? | [`migration::Migration::totality_over`] runs it across a fixture corpus. An empty corpus is *not* total — a migration nobody exercised is unchecked. |
//! | Is this migration lossy? | [`migration::Migration::audit_loss`] migrates forward and back. Anything that does not survive and was not declared is [`error::MigrationError::UndeclaredLoss`]. |
//! | May this field be removed? | [`deprecation::DeprecationLedger::audit_removals`] refuses a removal whose lifecycle has not finished, and a removal with no record at all gets its own error. |
//! | Does the descriptor match reality? | [`known`] describes the three shipped wire formats, and its tests hold each descriptor against the code that emits the bytes. |
//!
//! # What is deliberately not implemented
//!
//! - **No database.** 40.37 lists "partial DB migration" as a failure mode. It cannot occur here:
//!   there is no store, no transaction, no rollback. A migration is a pure function on a JSON
//!   value.
//! - **No migration runner.** Nothing walks a directory, streams a table, or writes a file. The
//!   projection-rebuild CLI 40.37 mentions would be a caller of this crate.
//! - **No clock.** 14.16's "minimum window" advances on an operator-supplied epoch, not on wall
//!   time, so that a lifecycle is reproducible. Nothing here reads the system time.
//! - **No transport.** 43.35 wants negotiation over CLI, REST, MCP and A2A. [`mode::negotiate`] is
//!   the decision those transports would call; it has no wire protocol of its own.
//! - **No JSON Schema validator.** [`descriptor::SchemaDescriptor::check_document`] checks
//!   presence and coarse shape, which is the part that changes meaning between versions. Patterns,
//!   ranges and enums belong to the ingest gate, not the evolution contract.
//! - **No trust tiers and no release gates.** `bioprism-registry` owns pack promotion and
//!   `bioprism-conformance` owns test suites. This crate neither imports nor reimplements them.
//! - **No process.** Announcements, telemetry, appeals (14.17) and the comparability study 14.16
//!   requires when scoring semantics change are human work. A type claiming to represent them
//!   would be claiming they happened.
//!
//! # Where the blueprint is silent
//!
//! 25.22 requires a "compatibility class" and never defines the classes. 14.16 promises that
//! "breaking changes use new namespace/version" without saying what makes a change breaking.
//! 40.37 takes "old/new schema versions" as an input without saying what a schema version is as a
//! value. The definitions this crate uses are stated at [`classify`] and [`descriptor`], in the
//! open, so that a reader can disagree with them rather than discover them.

pub mod classify;
pub mod deprecation;
pub mod descriptor;
pub mod diff;
pub mod error;
pub mod known;
pub mod migration;
pub mod mode;
pub(crate) mod pointer;
pub mod research_release_contract;
pub mod version;

pub use classify::{
    affects_digest, classify, classify_and_gate, ChangeVerdict, ClaimAudit, Classification,
    CompatibilityClass,
};
pub use deprecation::{
    DeprecationLedger, DeprecationRecord, LifecyclePolicy, Replacement, Stage, Transition,
};
pub use descriptor::{
    DigestRole, DocumentCheck, FieldSpec, FieldType, Mistyped, Presence, SchemaDescriptor,
};
pub use diff::{diff, FieldChange, SchemaDiff};
pub use error::{
    CompatibilityError, DeprecationError, DescriptorError, DigestBreach, GovernanceError,
    MigrationError, VersionError,
};
pub use migration::{
    DocumentFailure, LossAudit, LossItem, Migration, MigrationRegistry, MigrationStep,
    TotalityReport, SCHEMA_VERSION_KEY,
};
pub use mode::{apply_mode, negotiate, CompatibilityMode, Negotiated, UnknownFieldReport};
pub use research_release_contract::{
    compile_signed_research_object, research_release_contract_manifest, GovernanceReleaseError,
    SignedResearchObject, ValidatedResearchRun,
    CONTRACT_VERSION as RESEARCH_RELEASE_CONTRACT_VERSION,
    FEATURE_ID as RESEARCH_RELEASE_CONTRACT_FEATURE_ID,
};
pub use version::{observed_bump, SchemaId, SchemaVersion, VersionBump};
