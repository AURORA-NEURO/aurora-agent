//! Result bundles, symmetric and public-key attestation, and reproduction verdicts.
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
//! authenticates the manifest. Because the manifest binds every entry digest, one authentication
//! value over the manifest covers the whole closure. HMAC remains available for cooperating
//! processes; [`PubliclyAttestedBundle`] adds an explicit Ed25519 path for third-party verification.
//!
//! # The limitation, which is the point
//!
//! The two authentication paths have deliberately different claims. HMAC-SHA256 is symmetric: a
//! verifier needs the producer's secret and anyone who can verify can also forge. Ed25519 uses a
//! private signing seed and a separate public verification key, so a third party can verify the
//! signature without receiving signing material. Neither path turns a caller-supplied key label or
//! producer string into an authenticated organization identity.
//!
//! That is stated in the type system, not only in prose:
//!
//! | Rule | How it is enforced |
//! |---|---|
//! | The wire names its primitive | [`AuthenticationScheme`] distinguishes HMAC from Ed25519 |
//! | A verification result states forgeability | [`Repudiability`] distinguishes shared-secret and public-key paths |
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
//! - **No external identity authority.** [`trust`] can evaluate a bounded caller-supplied registry
//!   with signed delegation, roles, rotations, and revocations, but importing that snapshot does
//!   not make it an independent organization, transparency, or release authority.
//! - **No legal non-repudiation.** Public-key signatures make verifier-side forgery infeasible under
//!   the algorithm's security assumptions, but this crate does not establish custody, compromise
//!   history, authorization, or legal attribution of a key to a person.
//! - **No key management.** No generation, derivation, agreement, storage, rotation or escrow. A key
//!   is bytes the caller supplies.
//! - **No remote key lifecycle service.** [`trust`] applies signed lifecycle records from the
//!   caller's snapshot, while there is no CRL/OCSP-equivalent transport, remote rotation store,
//!   compromise feed, or authoritative clock.
//! - **No transparency log.** 13.20 §Transparency asks for a public log of releases, withdrawals and
//!   revocations. [`audit`] provides a hash-linked local log, which gives tamper *evidence* against
//!   a party without the key and nothing at all against a party with it. There is no witness, no
//!   gossip protocol, no inclusion proof against an external log and no Merkle consistency proof.
//! - **No timestamping authority.** This crate reads no clock. Every time value in a bundle is a
//!   string the caller asserted, and nothing corroborates it.
//! - **No organization-wide role authority.** [`trust::TrustPolicy`] can require publisher,
//!   builder, hub, auditor, or reproducer roles and can enforce signed attenuation, but the local
//!   policy is not a universal release-controller decision.
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
//! Ed25519 is the local scheme, selected for deterministic offline verification and a small public
//! key representation. [`trust`] fills the offline delegation, role, rotation, and revocation
//! policy seam, while cross-signing, HSM custody of a private key, timestamp authority, and a
//! transparency log remain separate capabilities and remain visible as limitations in higher-level
//! projections.
//!
//! # What the examples catalogue can now exercise
//!
//! `PubliclyAttestedBundle` makes the signed-result-bundle claim executable: a bundle verifies
//! independently of the compiler and without giving the verifier private signing material. The
//! producer-name, external identity authority, timestamp-authority, and external-closure portions
//! of the claim remain deliberately outside this crate. [`AttestedBundle`] remains available for
//! the symmetric shared-secret compatibility path.

pub mod attestation;
pub mod research_bundle_integrity_support;
pub mod local_research_bundle_integrity_inference;
pub mod multimodal_research_bundle_integrity_inference;
pub mod throughput_research_bundle_integrity_inference;
pub mod federated_continual_research_bundle_integrity_inference;
pub mod local_research_bundle_integrity_contract_model;
pub mod multimodal_research_bundle_integrity_contract_model;
pub mod throughput_research_bundle_integrity_contract_model;
pub mod federated_continual_research_bundle_integrity_contract_model;
pub mod local_research_bundle_integrity_research_copilot;
pub mod multimodal_research_bundle_integrity_research_copilot;
pub mod throughput_research_bundle_integrity_research_copilot;
pub mod federated_continual_research_bundle_integrity_research_copilot;
pub mod local_research_bundle_integrity_workflow_fabric;
pub mod multimodal_research_bundle_integrity_workflow_fabric;
pub mod throughput_research_bundle_integrity_workflow_fabric;
pub mod federated_continual_research_bundle_integrity_workflow_fabric;
pub mod audit;
pub mod bundle;
pub mod environment;
pub mod error;
mod hex;
pub mod mac;
pub mod manifest;
pub mod provenance;
pub mod reproduce;
pub mod signature;
pub mod trust;
pub mod retrieval_bundle_assurance;

pub use attestation::{
    Attestation, AttestationCheck, AttestationPurpose, ClaimedProducer, KeyHolderAuthenticated,
    ATTESTATION_SCHEMA_VERSION,
};
pub use research_bundle_integrity_support::{
    BundleArtifact4, BundleCard7, BundleEntry4, BundleReleaseRequest4,
    ResearchBundleIntegrityError,
};
pub use audit::{
    AuditAction, AuditCheckpoint, AuditEvent, AuditLog, AuditOutcome, ChainVerification,
    LinkedEntry, AUDIT_SCHEMA_VERSION,
};
pub use bundle::{
    AttestedBundle, BundleBuilder, EmbeddedCertificate, EntryCheck, PubliclyAttestedBundle,
    ResultBundle, VerifiedBundle,
};
pub use environment::{EnvironmentFacts, FactSource, ToolchainDifference, ToolchainFacts};
pub use error::BundleError;
pub use mac::{
    AuthenticationScheme, KeyIdentity, MacError, MacTag, Repudiability, SecretKey, TAG_SIZE,
};
pub use manifest::{BundleManifest, EntryBody, EntryRole, ManifestEntry, BUNDLE_SCHEMA_VERSION};
pub use provenance::{
    ProvenanceState, RecordedProvenance, RejectedProvenance, RejectionReason, SupplyChainPosture,
};
pub use reproduce::{
    Divergence, NotAttemptedReason, Reproduced, ReproductionAttempt, ReproductionVerdict,
    ToolchainPolicy,
};
pub use signature::{
    Ed25519PublicKey, Ed25519Signature, KeyValidity, PublicKeyAttestation,
    PublicKeyAttestationCheck, SignatureError, SigningKey, VerificationKey,
    PUBLIC_KEY_ATTESTATION_SCHEMA_VERSION,
};
pub use trust::{
    KeyDelegation, KeyRegistry, KeyRevocation, KeyRole, KeyRotation, RegisteredKey, TrustError,
    TrustPolicy, TrustReport, TrustVerdict, MAX_DELEGATION_DEPTH, MAX_TRUST_EVENTS, MAX_TRUST_KEYS,
    TRUST_REGISTRY_SCHEMA_VERSION,
};
pub use retrieval_bundle_assurance::{
    assure_retrieval_bundle, retrieval_bundle_assurance_manifest, BundleAssuranceError,
    BundleEvidenceCandidate, BundleEvidenceSynthesis, BundlePeerSummary, BundleRetrievalQuery,
    BundleSynthesisDisposition,
    CONTRACT_VERSION as RETRIEVAL_BUNDLE_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_BUNDLE_ASSURANCE_FEATURE_ID,
};
