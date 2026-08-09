//! The typed failure classes.
//!
//! 40.16 lists five: `dependency conflict`, `undeclared effect`, `conformance failure`,
//! `signature invalid`, `plugin revoked`. Four of them appear here under names that say what was
//! actually checked. `signature invalid` does not: this crate verifies no signatures, because it
//! installs no packages and fetches no bytes (see the crate docs). A `SignatureInvalid` variant
//! that nothing could ever construct would advertise a check that does not happen.
//!
//! Every variant names both sides of the disagreement wherever there are two. An error that says
//! "version mismatch" without the versions, or "capability conflict" without the two plugins,
//! moves the diagnosis back into a debugger.

use thiserror::Error;

/// Failures of the version handshake (43.35 versioning contract, 23.49 handshake).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NegotiationError {
    #[error("{text:?} is not a schema version; expected family/major.minor[-qualifier]")]
    MalformedVersion { text: String },

    #[error("the host declares no supported schema versions, so nothing can be negotiated")]
    HostSpeaksNothing,

    #[error("plugin declares no schema versions; host speaks {host_supported}")]
    PluginSpeaksNothing { host_supported: String },

    #[error(
        "no common schema version: host speaks {host_supported}; plugin speaks \
         {plugin_supported}{}",
        near_miss.as_ref().map(|d| format!(" ({d})")).unwrap_or_default()
    )]
    NoCommonVersion {
        host_supported: String,
        plugin_supported: String,
        /// Two versions of the same profile line, when there are any. Remediation, not a fallback.
        near_miss: Option<String>,
    },
}

/// A manifest that contradicts itself, checked before any registry is involved.
///
/// These are all properties of one manifest read alone, which is why they are separate from
/// [`RegistrationError`]: a plugin author can run [`crate::PluginManifest::validate`] without a
/// host and get the same answer the host would give.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ManifestError {
    #[error("a plugin needs a non-empty name")]
    EmptyName,

    #[error("plugin {plugin:?} declares an empty version")]
    EmptyVersion { plugin: String },

    #[error("plugin {plugin:?} declares no capabilities; there is nothing to register it for")]
    NoCapabilities { plugin: String },

    #[error("plugin {plugin:?} declares the {kind} capability twice")]
    DuplicateCapabilityKind { plugin: String, kind: String },

    #[error(
        "plugin {plugin:?} provides an adapter capability without a semantic-loss declaration; \
         an adapter that says nothing about loss has not audited it"
    )]
    AdapterWithoutLossDeclaration { plugin: String },

    #[error(
        "plugin {plugin:?} claims lossless ingestion while declaring loss kinds [{kinds}]; one of \
         the two statements is wrong"
    )]
    ContradictoryLossDeclaration { plugin: String, kinds: String },

    #[error(
        "plugin {plugin:?} claims determinism while declaring the {effect} effect, which is not \
         reproducible across runs"
    )]
    DeterminismContradictedByEffect { plugin: String, effect: String },

    #[error(
        "plugin {plugin:?} attaches conformance evidence for the {kind} capability, which it does \
         not declare"
    )]
    EvidenceForUndeclaredCapability { plugin: String, kind: String },

    #[error("plugin {plugin:?} cannot be canonically serialised for digesting: {detail}")]
    NotDigestible { plugin: String, detail: String },
}

/// Failures of registration itself: one manifest meeting a registry that already holds others.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistrationError {
    #[error(transparent)]
    Manifest(#[from] ManifestError),

    #[error(transparent)]
    Negotiation(#[from] NegotiationError),

    #[error(
        "plugin name {name:?} is already registered at version {incumbent_version:?}; this \
         registry does not allow version coexistence"
    )]
    DuplicateName {
        name: String,
        incumbent_version: String,
    },

    #[error("plugin {name:?} version {version:?} is already registered (digest {incumbent})")]
    DuplicateIdentity {
        name: String,
        version: String,
        incumbent: String,
    },

    #[error(
        "capability {kind} is claimed at priority {priority} by both {first} and {second}; \
         resolution would depend on registration order"
    )]
    CapabilityConflict {
        kind: String,
        priority: u32,
        /// The lexicographically smaller identity, so the message does not depend on which
        /// plugin was offered first.
        first: String,
        second: String,
    },

    #[error("plugin {plugin} requires the {effect} effect, which this host does not grant")]
    EffectNotPermitted { plugin: String, effect: String },
}

/// Failures of asking a registered plugin to do something.
///
/// Separate from registration because 40.16's first invariant is that registration does not imply
/// trust. Everything here is a check that happens *after* a plugin is legitimately in the
/// registry, at the moment a host tries to use it for a particular job.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SelectionError {
    #[error("no plugin is registered for the {kind} capability")]
    NoProviderFor { kind: String },

    #[error("no plugin registered for the {kind} capability provides {subject:?}")]
    NoProviderForSubject { kind: String, subject: String },

    #[error("plugin {plugin:?} is not registered")]
    UnknownPlugin { plugin: String },

    #[error(
        "plugin {plugin} is at trust level {trust} and cannot be selected for load-bearing work: \
         {reasons}"
    )]
    NotSelectableForLoadBearing {
        plugin: String,
        trust: String,
        reasons: String,
    },

    #[error(
        "plugin {plugin} declares ABI grade {declared} but {ask} requires at least {required}"
    )]
    AbiGradeTooLow {
        plugin: String,
        ask: String,
        declared: String,
        required: String,
    },

    #[error("plugin {plugin} is revoked: {reason}")]
    Revoked { plugin: String, reason: String },
}
