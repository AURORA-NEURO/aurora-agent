//! Typed failures. Every variant names the thing that failed, not just the fact of failure.
//!
//! Blueprint 13.15 §Failure modes asks implementations to "emit an actionable diagnostic rather than
//! silently repairing or discarding state". An error that says "manifest mismatch" is not actionable;
//! one that says which entry, what was claimed and what was recomputed is.

use crate::mac::{KeyIdentity, MacError};
use bioprism_ids::CanonicalError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BundleError {
    /// A manifest entry's recorded digest disagrees with the content the bundle actually carries.
    ///
    /// This is the failure the whole verify path exists to catch: a manifest is a claim about
    /// content, and verification recomputes rather than trusts it.
    #[error(
        "bundle entry `{entry}` does not match its manifest digest: manifest says {claimed}, \
         recomputing the carried content gives {recomputed}"
    )]
    EntryDigestMismatch {
        entry: String,
        claimed: String,
        recomputed: String,
    },

    /// The manifest's own digest disagrees with the manifest bytes.
    #[error(
        "manifest digest {claimed} does not match the manifest body, which hashes to {recomputed}"
    )]
    ManifestDigestMismatch { claimed: String, recomputed: String },

    /// Two entries share a name, so "the digest of `x`" is ambiguous.
    #[error("bundle contains two entries named `{entry}`; entry names index the manifest and must be unique")]
    DuplicateEntry { entry: String },

    /// A role that 34.14 requires exactly one of appeared zero times or more than once.
    #[error("bundle needs exactly one entry with role `{role}`, found {found}")]
    RoleCardinality { role: String, found: usize },

    /// A bundle carries content under a name the manifest does not list.
    #[error("bundle carries content named `{entry}` that the manifest does not list; an unlisted payload is outside the authenticated closure")]
    UnlistedContent { entry: String },

    /// A manifest lists an inline entry whose content is absent.
    #[error(
        "manifest lists `{entry}` as carried inline but the bundle has no content under that name"
    )]
    MissingInlineContent { entry: String },

    /// A carried Context Certificate failed 43.26's own self-verification.
    ///
    /// Separate from [`BundleError::EntryDigestMismatch`]: the entry's digest can match the carried
    /// bytes perfectly while those bytes are a certificate whose `certificate_sha256` disagrees with
    /// its own body. A bundle that faithfully carries a broken certificate is still broken.
    #[error("the Context Certificate carried as `{entry}` does not verify: {detail}")]
    EmbeddedCertificateInvalid { entry: String, detail: String },

    /// The tag does not verify under the offered key.
    #[error("attestation tag does not verify under key `{key_identity}`; the bytes were altered, or the key is not the one that produced it — these are indistinguishable")]
    TagMismatch { key_identity: KeyIdentity },

    /// An Ed25519 signature does not verify under the offered public key.
    #[error("ed25519 signature does not verify under public key `{key_identity}`")]
    SignatureMismatch { key_identity: KeyIdentity },

    /// The attestation names one key and the verifier offered another.
    #[error("attestation was produced under key `{attested}` but verification was offered key `{offered}`")]
    KeyIdentityMismatch {
        attested: KeyIdentity,
        offered: KeyIdentity,
    },

    /// A public key was not valid for the caller-supplied signing instant.
    #[error("public key `{key_identity}` is not valid at signing instant {signed_at:?}: {detail}")]
    KeyNotValidAtSigningTime {
        key_identity: KeyIdentity,
        signed_at: Option<u64>,
        detail: String,
    },

    /// A bounded validity window cannot be applied without a caller-supplied signing instant.
    #[error("public key `{key_identity}` has a bounded validity window but the attestation has no signing instant")]
    MissingSigningTime { key_identity: KeyIdentity },

    /// The attestation covers a different manifest than the bundle presents.
    #[error(
        "attestation covers manifest {attested} but this bundle's manifest hashes to {actual}"
    )]
    AttestationCoversDifferentManifest { attested: String, actual: String },

    /// The attestation could not be reduced to canonical bytes, so no check was possible.
    ///
    /// Distinct from a failing tag: nothing was checked, rather than checked and rejected.
    #[error(
        "attestation could not be reduced to canonical bytes, so no check was performed: {detail}"
    )]
    AttestationUnreadable { detail: String },

    /// A tag was replayed from a different attestation purpose.
    #[error("attestation was produced for purpose `{attested}` and cannot be reused for purpose `{requested}`")]
    PurposeMismatch { attested: String, requested: String },

    /// The audit chain's link digests do not form an unbroken sequence.
    #[error("audit chain breaks at sequence {sequence}: entry records previous={recorded} but the preceding entry hashes to {computed}")]
    AuditChainBroken {
        sequence: u64,
        recorded: String,
        computed: String,
    },

    /// Canonical serialization failed, which for JSON built by this crate means a non-finite float
    /// reached a bundle payload.
    #[error("canonical serialisation failed: {0}")]
    Canonical(#[from] CanonicalError),

    /// A tag could not be parsed from its wire form.
    #[error(transparent)]
    Mac(#[from] MacError),
}
