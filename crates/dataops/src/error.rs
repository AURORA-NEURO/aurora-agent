//! Typed refusals, one enum per module.
//!
//! Split by module rather than unified so that a function's signature says which vocabulary of
//! failure it speaks. A single `DataOpsError` would let a catalog constraint violation and an
//! isolation-adequacy refusal reach the same `match` arm, and the second is a security decision.
//!
//! Every variant names the thing it refused and the value that caused it. None of them is a
//! recoverable-by-retry condition; this crate has no I/O, so every error here is a statement
//! about the inputs.

use serde_json::Value;
use thiserror::Error;

/// Failures constructing a provenance record.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum BasisError {
    #[error("malformed {field}: {value:?}")]
    MalformedField {
        field: &'static str,
        value: String,
    },
    /// More observations than the population admits.
    ///
    /// Refused rather than clamped: the population figure is wrong, and a clamp would report
    /// `Complete` for the one case where the counting is known to be broken.
    #[error("observed {observed} of a population of {expected}")]
    CoverageExceedsPopulation { observed: u64, expected: u64 },
}

/// Failures assembling or checking a 12.02 storage topology.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum TopologyError {
    #[error("malformed {field}: {value:?}")]
    MalformedField {
        field: &'static str,
        value: String,
    },
    #[error("store {name:?} is declared twice")]
    DuplicateStore { name: String },
    #[error("data class {class} is assigned to store {store:?}, which is not declared")]
    UndeclaredStore { class: &'static str, store: String },
    #[error("data class {class} has no store assignment")]
    ClassUnassigned { class: &'static str },
    #[error("data class {class} holds immutable evidence but store {store:?} permits rewriting")]
    EvidenceStoreIsMutable { class: &'static str, store: String },
    #[error("store {store:?} is rebuildable from {from}, which has no canonical store")]
    RebuildSourceNotCanonical { store: String, from: &'static str },
    #[error("store {store:?} claims to rebuild from itself")]
    RebuildCycle { store: String },
    #[error("data class {class} is held only by rebuildable store {store:?}")]
    NoCanonicalHolder { class: &'static str, store: String },
}

/// Failures against the 12.03 catalog constraints.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CatalogError {
    #[error("malformed {field}: {value:?}")]
    MalformedField {
        field: &'static str,
        value: String,
    },
    #[error("no object {object:?}")]
    UnknownObject { object: String },
    #[error("no revision {revision:?}")]
    UnknownRevision { revision: String },
    #[error("digest {digest:?} for media type {media_type:?} is already revision {existing:?}")]
    DuplicateDigest {
        media_type: String,
        digest: String,
        existing: String,
    },
    #[error("alias {alias:?} in scope {scope:?} would resolve into namespace {target_namespace:?}")]
    AliasCrossesNamespace {
        alias: String,
        scope: String,
        target_namespace: String,
    },
    #[error("publication {publication:?} already exists")]
    DuplicatePublication { publication: String },
    #[error("publication {publication:?} needs {missing:?}, which is not in the catalog")]
    ClosureIncomplete {
        publication: String,
        missing: String,
    },
    #[error("publication {publication:?} depends on withdrawn revision {revision:?}")]
    ClosureWithdrawn {
        publication: String,
        revision: String,
    },
    #[error("{reference:?} refers to {target:?}, which is not in the catalog")]
    DanglingReference { reference: String, target: String },
    #[error("revision {revision:?} is referenced by publication {publication:?}")]
    ReferencedByPublication {
        revision: String,
        publication: String,
    },
    #[error("lineage edge {child:?} -> {parent:?} closes a cycle")]
    LineageCycle { child: String, parent: String },
    #[error("outbox cursor {cursor} is ahead of the {emitted} events emitted")]
    OutboxCursorAhead { cursor: u64, emitted: u64 },
    #[error("audit event could not be built: {detail}")]
    Audit { detail: String },
}

/// Failures defining or evaluating a 12.12 objective.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum SloError {
    #[error("malformed {field}: {value:?}")]
    MalformedField {
        field: &'static str,
        value: String,
    },
    /// A target of the form good/total with total zero, or good above total.
    #[error("objective {name:?} has an impossible target {good}/{total}")]
    ImpossibleTarget {
        name: String,
        good: u64,
        total: u64,
    },
    #[error("objective {name:?} is declared twice")]
    DuplicateObjective { name: String },
    #[error("no objective {name:?}")]
    UnknownObjective { name: String },
    #[error("window ends at epoch {end} before it starts at {start}")]
    WindowInverted { start: u64, end: u64 },
}

/// Failures declaring or admitting a 12.13 compute provider.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ProviderError {
    #[error("malformed {field}: {value:?}")]
    MalformedField {
        field: &'static str,
        value: String,
    },
    #[error("provider {provider:?} is declared twice")]
    DuplicateProvider { provider: String },
    #[error("no provider {provider:?}")]
    UnknownProvider { provider: String },
    /// The provider's isolation is weaker than the threat level requires.
    #[error("provider {provider:?} offers {offered} isolation; {threat} work requires at least {required}")]
    IsolationInadequate {
        provider: String,
        offered: &'static str,
        required: &'static str,
        threat: &'static str,
    },
    /// The policy required a conformance result the platform measured, and got the provider's own
    /// claim instead.
    #[error("provider {provider:?} conformance is {basis}, and the policy requires a first-hand result")]
    ConformanceNotVerified { provider: String, basis: String },
    #[error("warm pool in trust domain {pool:?} cannot serve work from {job:?}")]
    TrustDomainMismatch { pool: String, job: String },
    #[error("the local path requires {service}, which 12.13 says it must not")]
    LocalPathNeedsExternalService { service: &'static str },
}

/// Failures placing 12.14 work on workers.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum PlacementError {
    #[error("malformed {field}: {value:?}")]
    MalformedField {
        field: &'static str,
        value: String,
    },
    #[error("worker {worker:?} is declared twice")]
    DuplicateWorker { worker: String },
    #[error("no worker {worker:?}")]
    UnknownWorker { worker: String },
    /// Speculative duplication of work that is not safe to run twice.
    #[error("task {task:?} is not idempotent, so it cannot be speculatively duplicated")]
    SpeculationUnsafe { task: String },
    #[error("lease for {task:?} expired at epoch {expires_at}; the result arrived at {arrived_at}")]
    LeaseExpired {
        task: String,
        expires_at: u64,
        arrived_at: u64,
    },
    #[error("shard count {shards} cannot cover {units} units")]
    ImpossibleShardCount { shards: u64, units: u64 },
    #[error("seed derivation failed: {detail}")]
    Seed { detail: String },
}

/// Failures in a 12.15 local-first deployment.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum LocalError {
    #[error("malformed {field}: {value:?}")]
    MalformedField {
        field: &'static str,
        value: String,
    },
    #[error("probe {probe:?} is declared twice")]
    DuplicateProbe { probe: String },
    #[error("requirement {requirement:?} needs host {host:?} and the offline contract is closed")]
    NetworkDenied { requirement: String, host: String },
    #[error("host {host:?} is not on the allow list")]
    HostNotAllowed { host: String },
    #[error("{resource} needs {needed} but the envelope declares {available}")]
    EnvelopeExceeded {
        resource: &'static str,
        needed: u64,
        available: u64,
    },
}

/// Failures in a 12.16 cloud or federated deployment plan.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum FederationError {
    #[error("malformed {field}: {value:?}")]
    MalformedField {
        field: &'static str,
        value: String,
    },
    #[error("plane {plane} has no placement")]
    PlaneUnplaced { plane: &'static str },
    #[error("plane {plane} is placed in a customer network, which the {pattern} pattern forbids")]
    PlaneMisplaced {
        plane: &'static str,
        pattern: &'static str,
    },
    /// A plan that needs the hub to open a connection into a customer network.
    #[error("plane {plane} would require an inbound connection into a customer network")]
    InboundRequired { plane: &'static str },
    #[error("artifact {artifact:?} is sensitive and pinned to {region:?}; it cannot be replicated")]
    SensitiveArtifactReplication { artifact: String, region: String },
    #[error("hub {hub:?} is not a trusted publisher")]
    UntrustedPublisher { hub: String },
    #[error("record from {hub:?} carries no attestation and the policy does not accept unattested imports")]
    UnattestedImport { hub: String },
}

impl From<bioprism_ids::CanonicalError> for PlacementError {
    fn from(value: bioprism_ids::CanonicalError) -> Self {
        PlacementError::Seed {
            detail: value.to_string(),
        }
    }
}

impl From<bioprism_ledger::LedgerError> for CatalogError {
    fn from(value: bioprism_ledger::LedgerError) -> Self {
        CatalogError::Audit {
            detail: value.to_string(),
        }
    }
}

/// Shared field validation.
///
/// Every identifier in this crate is a non-empty control-character-free string, checked at
/// construction so that no downstream digest ever commits to a value that would not round-trip
/// through a log line.
pub(crate) fn check_name(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(char::is_control)
}

/// A JSON payload rendered for an error message without dragging the whole value into the enum.
pub(crate) fn describe(value: &Value) -> String {
    let text = value.to_string();
    if text.len() <= 96 {
        text
    } else {
        format!("{}…", &text[..96])
    }
}
