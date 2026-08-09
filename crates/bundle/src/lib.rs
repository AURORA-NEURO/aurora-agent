//! Result bundles, symmetric attestation and reproduction verdicts.
//!
//! Implements blueprint 34.14 (Result Cards, Signed Bundles and Reproduction), with 13.15 (supply
//! chain and artifact security) for input provenance and 13.20 (audit log transparency and
//! attestation) for the hash-linked event log.
//!
//! # What a bundle adds to a certificate
//!
//! A Context Certificate (43.26, `bioprism-section`) proves *integrity*: recompute the canonical
//! hash of the body and it matches the `certificate_sha256` printed on it. It proves nothing about
//! *origin*. Anyone can invent a world, compile a context from it, and emit a perfectly valid
//! certificate for the result. A certificate says "these bytes are self-consistent". It does not say
//! "these bytes came from anywhere in particular".
//!
//! A result bundle wraps a certificate with the rest of what a reproduction needs — the decision
//! section, the world and query digests, the toolchain and crate versions, declared environment
//! facts, and the recorded provenance of every input — hashes all of it into a manifest, and
//! authenticates the manifest. Because the manifest binds every entry digest, one tag over the
//! manifest covers the whole closure.
//!
//! # The limitation, which is the point
//!
//! This workspace builds offline against pinned dependencies and cannot add an external crate. The
//! only cryptographic primitive available is `sha2`, from which one can build HMAC-SHA256 and
//! nothing asymmetric. So this crate offers **symmetric authentication only**, and the platform
//! consequently **cannot offer third-party verifiability**. A verifier needs the very secret the
//! producer used; anyone who can verify can also forge.
//!
//! That is stated in the type system, not only in prose:
//!
//! | Rule | How it is enforced |
//! |---|---|
//! | No value can claim asymmetric signing | [`AuthenticationScheme`] has one variant, `SymmetricSharedSecret` |
//! | No value can claim non-repudiation | [`Repudiability`] has one variant, `ForgeableByAnyVerifier` |
//! | A tag cannot be quoted as a signature | [`MacTag`] always renders with its `hmac-sha256:` prefix |
//! | A verified result cannot name a party | [`KeyHolderAuthenticated`] has private fields and no accessor for the claimed producer |
//! | Environment facts cannot claim to be measured | [`FactSource`] has one variant, `DeclaredByCaller` |
//! | Unknown provenance is not bad provenance | [`ProvenanceState::Unrecorded`] and `::Rejected` are separate variants with no collapsing helper |
//! | A reproduction cannot claim more than it compared | [`Reproduced`] carries `not_compared` alongside `compared` |
//!
//! # Deliberately not implemented
//!
//! This list is load-bearing. A missing capability that is stated is a limitation; one that is
//! implied to exist is a lie.
//!
//! - **No public-key signatures.** No RSA, no ECDSA, no Ed25519, no post-quantum scheme. There is no
//!   big-integer type, no field arithmetic and no curve in the dependency set.
//! - **No third-party verifiability.** Verification requires the producing secret. A reader who does
//!   not hold the key learns nothing from a tag; a reader who does hold it could have written it.
//! - **No non-repudiation.** A producer denying authorship is not contradicted by anything here.
//! - **No key management.** No generation, derivation, agreement, storage, rotation or escrow. A key
//!   is bytes the caller supplies.
//! - **No revocation and no key validity window.** 13.20 §Verification asks a CLI to check "key
//!   validity at signing time, and revocation status". Neither is possible: there is no CRL, no
//!   OCSP-equivalent, no key expiry field with anything to enforce it, and no clock.
//! - **No transparency log.** 13.20 §Transparency asks for a public log of releases, withdrawals and
//!   revocations. [`audit`] provides a hash-linked local log, which gives tamper *evidence* against
//!   a party without the key and nothing at all against a party with it. There is no witness, no
//!   gossip protocol, no inclusion proof against an external log and no Merkle consistency proof.
//! - **No timestamping authority.** This crate reads no clock. Every time value in a bundle is a
//!   string the caller asserted, and nothing corroborates it.
//! - **No signature purpose separation in the cryptographic sense.** 13.15 §Signing wants publisher,
//!   builder and hub signatures to be purpose-separated. [`attestation::AttestationPurpose`] labels
//!   the purpose inside the authenticated bytes, which prevents cross-purpose *replay* of a tag but
//!   is not key separation, because there is one key.
//! - **No scanning, no sandboxing, no build isolation.** 13.15 §Scanning and §Build isolation are
//!   out of scope for a library of plain Rust types; this crate records provenance, it does not
//!   establish it.
//! - **No content storage or transport.** A bundle is a value. Nothing here writes a file, opens a
//!   socket or fetches a referenced artifact.
//! - **No clock and no RNG.** Every nonce, key, identity and timestamp is supplied by the caller, so
//!   the same inputs always produce the same bundle bytes and the same tag.
//!
//! # Where the blueprint assumes a signing scheme it never specifies
//!
//! Recorded here because a later implementer will hit it too, and because it is the reason this
//! crate had to choose a construction rather than follow one.
//!
//! Signing is load-bearing across the specification and is nowhere defined. 34.14 lists "attestation
//! signatures" as a primary capability and "signature verification" as a product metric. 13.15
//! §Signing assigns three separate signing roles — publisher, trusted builder, hub — and requires
//! that "signatures are purpose-separated". 13.20 §Verification asks a CLI to check "signatures, log
//! inclusion, artifact closure, **key validity at signing time, and revocation status**". 10.02 makes
//! signing keys a layer of the registry; 10.19 has organizations "cross-sign collaborators"; 13.07
//! says "authentication identity and artifact signing identity may differ but are linked through
//! explicit delegation"; 13.08 §Signing keys puts release and attestation keys "in hardware-backed
//! stores or user custody"; 12.19 keeps private keys in "user/organization custody or HSM" with
//! rotation and revocation records. ADR-011, "Accepted for blueprint", decides that published
//! artifacts "will be content-addressed and signed".
//!
//! Every one of those properties — verification by a party who cannot sign, delegation, cross-signing,
//! HSM custody of a *private* key, revocation, a transparency log — presupposes an **asymmetric**
//! scheme. None of them means anything under a shared secret.
//!
//! The scheme itself is chosen nowhere. The only algorithm named anywhere in the distribution is in
//! 23.38's WeaveIR event example, a JSON literal `"signature": {"method": "ed25519"}` in a module
//! whose owner is "Unassigned" — an illustration, not a normative choice. 21.07 states the position
//! outright: "Sigstore-style signing **may be evaluated; final choice requires ADR and local/offline
//! support**." No such ADR exists in `20_ARCHITECTURE_DECISIONS`, and ADR-011's own follow-up list
//! still reads "implement transparency-log integration" and "document revocation and key-rotation
//! policy".
//!
//! So the specification requires signatures, describes an ecosystem that only an asymmetric scheme
//! can support, and defers the choice of scheme to a decision record that was never written. This
//! crate does not fill that gap — it cannot, offline against `sha2` — and it declines to paper over
//! it. What it does instead is make the gap unmissable in the type system, so that whoever writes
//! that ADR inherits a crate that never claimed the property it was missing.
//!
//! # What the examples catalogue can now exercise
//!
//! `bioprism-examples` records `signed_result_bundle_replay` as blocked because "no signing or
//! bundle crate is in this crate's dependency set; certificates are content-addressed but unsigned".
//! The bundle half of that obstacle is now gone. The claim's own wording — "a signed result bundle
//! verifies **independently** of the runtime that produced it" — remains only half reachable: a
//! bundle verifies independently of the *compiler* (`bioprism-section` deliberately does not depend
//! on `bioprism-fiber`, and neither does this crate), but not independently of the *producer*, who
//! must share their key with the verifier. See [`attestation`] for the exact statement.

pub mod attestation;
pub mod audit;
pub mod bundle;
pub mod environment;
pub mod error;
pub mod mac;
pub mod manifest;
pub mod provenance;
pub mod reproduce;

pub use attestation::{
    Attestation, AttestationCheck, AttestationPurpose, ClaimedProducer, KeyHolderAuthenticated,
    ATTESTATION_SCHEMA_VERSION,
};
pub use audit::{
    AuditAction, AuditCheckpoint, AuditEvent, AuditLog, AuditOutcome, ChainVerification,
    LinkedEntry, AUDIT_SCHEMA_VERSION,
};
pub use bundle::{
    AttestedBundle, BundleBuilder, EmbeddedCertificate, EntryCheck, ResultBundle, VerifiedBundle,
};
pub use environment::{EnvironmentFacts, FactSource, ToolchainDifference, ToolchainFacts};
pub use error::BundleError;
pub use mac::{
    AuthenticationScheme, KeyIdentity, MacError, MacTag, Repudiability, SecretKey, TAG_SIZE,
};
pub use manifest::{
    BundleManifest, EntryBody, EntryRole, ManifestEntry, BUNDLE_SCHEMA_VERSION,
};
pub use provenance::{
    ProvenanceState, RecordedProvenance, RejectedProvenance, RejectionReason, SupplyChainPosture,
};
pub use reproduce::{
    Divergence, NotAttemptedReason, ReproductionAttempt, ReproductionVerdict, Reproduced,
    ToolchainPolicy,
};
