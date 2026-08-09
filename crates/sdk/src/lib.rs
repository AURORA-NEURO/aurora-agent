//! Plugin registration and capability declaration.
//!
//! Implements blueprint 40.16 (Plugin SDK and Registration), 11.12 (Plugin and Extension System)
//! and 11.13 (Plugin SDK), the capability-discovery and version-negotiation contract of 43.35
//! (Schemas, APIs and the FIBER Cognitive ABI), and the ABI-P0..P4 portability grades of 23.49
//! (Interweave Microkernel and Cognitive ABI). The adapter-specific obligations come from 11.14
//! (Trace, Runner and Provider Adapter SDK), whose "measure semantic loss" responsibility this
//! crate enforces at registration rather than re-implementing.
//!
//! # This is the layer above the extension points, not another one
//!
//! The workspace already has three extension traits with three different shapes —
//! `bioprism_adapter::Adapter`, `bioprism_oracle::Oracle`, `bioprism_baseline::ContextStrategy` —
//! plus data-shaped mutation operators and `bioprism_backends::QueryBackend`. They are different
//! shapes because they do different jobs, and unifying them behind a fourth trait would mostly
//! consist of `Box<dyn Any>`.
//!
//! What they have in common is not a signature. It is a set of questions a host has to answer
//! before it calls any of them: *what is this thing, which wire version does it speak, what has
//! anybody checked about it, what does it need permission to do, and may it be given this
//! particular job?* Those questions are this crate. A [`PluginManifest`] answers them as
//! declarations; a [`PluginRegistry`] checks the answers, refuses the ones that contradict
//! themselves or each other, and resolves the rest deterministically.
//!
//! The consequence, stated plainly: **this crate never touches the implementation**. It cannot
//! verify that a plugin claiming [`CapabilityKind::Adapter`] implements `Adapter`, because it does
//! not link the adapter crate and holds no value of that type. It checks declarations against each
//! other and against the host's policy. Checking a declaration against behaviour is what a
//! conformance suite does, and its verdict arrives here as [`ConformanceEvidence`] — from
//! somebody, which is why who ran it is a field.
//!
//! # Declared, then checked — never inferred
//!
//! 43.35's invariant is that **permissions are not inferred from protocol connection**, and 40.16's
//! is that **registration does not imply trust**. Those two sentences produce most of the shape
//! here:
//!
//! * A plugin that registers successfully has proved nothing about itself. It has produced a
//!   self-consistent declaration that the host was willing to accept.
//! * A plugin with no [`ConformanceEvidence`] lands at [`TrustLevel::Unverified`] and
//!   [`PluginRegistry::resolve_load_bearing`] will not select it, however high a priority it
//!   declared. It can still be resolved for exploratory work, because refusing to register it at
//!   all would push unverified plugins out of the audit record rather than out of the critical
//!   path.
//! * A version that is not in the intersection of both sides' sets is refused with both sets named
//!   ([`negotiate`]). There is no optimistic attempt.
//! * An [`AbiGrade`] is a ceiling on what may be asked. [`PluginRegistry::ask`] refuses to hand a
//!   P0 plugin a continuation, which 23.49 singles out as the claim a low-grade participant must
//!   not be able to make falsely.
//!
//! # What this crate does not do
//!
//! Every item below is absent by decision, and each would be a lie if implied:
//!
//! * **No dynamic loading.** No `dlopen`, no shared objects, no plugin directories, no entry-point
//!   strings. A "plugin" here is code the host already compiled in; the manifest is how it
//!   describes itself.
//! * **No WebAssembly.** 23.49 suggests a Wasm component target and this crate is not it.
//! * **No process isolation, no sandbox, no resource limits.** [`SandboxDeclaration`] is recorded
//!   and enforced by nothing — see [`sandbox`] for the full statement. Declared-and-recorded is
//!   useful; declared-and-enforced would be a security boundary that does not exist.
//! * **No network, no filesystem, no discovery.** Nothing is fetched, installed or scanned. The
//!   [`Effect`] list is a declaration about what a plugin will do, made by the plugin.
//! * **No signature verification.** 40.16 lists `signature invalid` as a failure class; there is no
//!   such variant in [`error::RegistrationError`], because nothing here holds a key or a package.
//! * **No conformance execution.** Suites live in `bioprism-conformance` and the extension crates.
//!   This crate reads their verdicts and refuses to invent them.
//! * **No revocation feeds or trust roots.** [`PluginRegistry::revoke`] records a local decision.
//!
//! # Reading order
//!
//! [`capability`] for what a plugin can claim to be, [`version`] for the handshake, [`evidence`]
//! for how a claim earns trust, [`registry`] for the conflict and resolution rules.

pub mod abi;
pub mod capability;
pub mod discovery;
pub mod effect;
pub mod error;
pub mod evidence;
pub mod loss;
pub mod manifest;
pub mod registry;
pub mod sandbox;
pub mod version;

pub use abi::{check_ask, AbiGrade, HostAsk};
pub use capability::{Capability, CapabilityKind, Priority};
pub use discovery::{Availability, CapabilityCard, ProviderCard};
pub use effect::{Determinism, Effect, EffectClass, EffectPolicy};
pub use error::{ManifestError, NegotiationError, RegistrationError, SelectionError};
pub use evidence::{assess_trust, ConformanceEvidence, TrustAssessment, TrustLevel};
pub use loss::{SemanticLossDeclaration, KNOWN_LOSS_KINDS};
pub use manifest::{PluginId, PluginManifest};
pub use registry::{PluginRegistry, Registration, RegistryPolicy, Revocation};
pub use sandbox::{Enforcement, IsolationClass, ResourceRequest, SandboxDeclaration};
pub use version::{negotiate, Negotiated, SchemaVersion, VersionSet};

/// The sentence a host should be able to quote when asked what a registration proves.
///
/// Exists as a value rather than only as prose so that a CLI or a report can print it verbatim
/// next to a trust level, instead of each surface paraphrasing the caveat slightly differently
/// until one of them paraphrases it away.
pub fn conformance_note() -> &'static str {
    "Registration checks declarations against each other and against host policy. It does not \
     execute the plugin, verify signatures, or apply isolation. Behavioural claims come only from \
     conformance evidence, and evidence is attributed to whoever ran it."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_conformance_note_does_not_claim_isolation_or_verification() {
        let note = conformance_note();
        assert!(note.contains("does not"));
        assert!(note.contains("isolation"));
        assert!(note.contains("signatures"));
    }
}
