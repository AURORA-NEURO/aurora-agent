//! Packaging a slice's own compile as a result bundle, and handing it back to a verifier.
//!
//! Blueprint 19.06 asks that a result be judged from a bundle rather than from console output, and
//! 34.14 asks that a reader be able to check one without an account. This module is the half of
//! that a *reference example* can supply: it takes the certificate and section a slice just
//! compiled, wraps them with the query and a digest of the world, attests the manifest, sends the
//! whole thing through JSON, and verifies what comes back.
//!
//! # Why the bundle is built from a compile rather than from a literal
//!
//! `bioprism-bundle` has no compiler and cannot get one — it depends on `bioprism-ids` and
//! `bioprism-section` and deliberately not on `bioprism-fiber` — so its own tests must hand-write
//! the certificate they bundle. That is enough to test the bundle machinery and is not enough to
//! say that a *compiled* certificate survives the round trip: a certificate the test author wrote
//! is a certificate the test author can keep bundle-shaped. Here the bytes come out of
//! [`bioprism_fiber::compile`], so the round trip is the one a consumer would actually perform.
//!
//! # What this module deliberately does not claim
//!
//! Verification is independent of the *compiler* and not of the *producer*. The scheme is
//! HMAC-SHA256 under a shared secret, so a verifier needs the producing key and a verifier holding
//! it could have written the tag. Both limits are recorded as observations —
//! [`crate::BundleObservation::without_the_key`] and
//! [`crate::BundleObservation::verifier_forgery_is_identical`] — rather than left to prose, because
//! a bundle observation carrying only its successful verification reads as third-party
//! verifiability, which this workspace does not have.
//!
//! Nothing here writes a file or opens a socket, and no entry is fetched: the world travels as a
//! digest, which verification reports as *not recomputed* rather than as a passing check.

use crate::error::ExampleError;
use crate::expectation::BundleProbe;
use crate::report::BundleObservation;
use bioprism_bundle::{
    AttestationCheck, AttestationPurpose, AttestedBundle, ClaimedProducer, EmbeddedCertificate,
    EntryRole, KeyIdentity, ProvenanceState, RecordedProvenance, ResultBundle, SecretKey,
};
use bioprism_fiber::CompileOutput;
use bioprism_ids::ContentHash;
use serde_json::Value;

/// Entry names, fixed rather than configurable: a reader comparing two slice reports is comparing
/// the same four roles, and a per-slice naming scheme would make that comparison a lookup.
const CERTIFICATE: &str = "certificate";
const SECTION: &str = "section";
const QUERY: &str = "query";
const WORLD: &str = "world";

/// The documents a bundle is assembled from, alongside the compile that produced them.
pub struct BundleInputs<'a> {
    pub output: &'a CompileOutput,
    pub world_document: &'a Value,
    pub query_document: &'a Value,
    /// The digest of the serialised [`bioprism_worldgen::WorldSpec`] the world was generated from.
    ///
    /// This is the immutable revision 13.15 §Pinning asks for, and it is available here for a
    /// reason peculiar to this crate: a slice's world is not fetched from anywhere, it is a
    /// deterministic function of a spec, so the spec's bytes *are* the revision that fixes the
    /// world's bytes.
    pub spec_digest: &'a ContentHash,
}

/// Builds, attests, transports and verifies a bundle over one slice's compile.
pub fn run(
    slice: &str,
    probe: &BundleProbe,
    inputs: &BundleInputs<'_>,
) -> Result<BundleObservation, ExampleError> {
    let bundle_error = |source| ExampleError::Bundle {
        slice: slice.to_string(),
        source,
    };

    let key = || {
        SecretKey::new(
            KeyIdentity::new(probe.key_identity.clone()),
            probe.key_bytes.clone(),
        )
    };
    let produce = || {
        AttestedBundle::produce(
            assemble(probe, inputs).map_err(bundle_error)?,
            &key(),
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new(probe.claimed_producer.clone()),
        )
        .map_err(bundle_error)
    };

    let attested = produce()?;
    let wire = serde_json::to_string(&attested).expect("an attested bundle is serialisable");
    let received: AttestedBundle =
        serde_json::from_str(&wire).expect("a bundle this crate just serialised parses back");
    let survives_json_round_trip = received == attested;

    let (verified, authenticated) = received.verify(&key()).map_err(bundle_error)?;

    let stranger = SecretKey::new(
        KeyIdentity::new(format!("{}-reviewer", probe.key_identity)),
        vec![0x00; 32],
    );
    let without_the_key = outcome_word(&received.attestation.verify(&stranger));

    let recomputed_entries: Vec<String> = verified
        .entry_checks()
        .iter()
        .filter(|(_, check)| check.was_recomputed())
        .map(|(name, _)| name.clone())
        .collect();

    Ok(BundleObservation {
        bundle_id: probe.bundle_id.clone(),
        manifest_digest: verified.manifest_digest().as_str().to_string(),
        recomputed_entries,
        not_recomputed: verified
            .not_recomputed()
            .into_iter()
            .map(str::to_string)
            .collect(),
        embedded_certificate: certificate_word(verified.certificate()).to_string(),
        survives_json_round_trip,
        authenticated_key: authenticated.key_identity().as_str().to_string(),
        tag: received.attestation.tag.to_string(),
        scheme: authenticated.scheme().to_string(),
        repudiability: authenticated.repudiability().to_string(),
        without_the_key: without_the_key.to_string(),
        verifier_forgery_is_identical: produce()? == attested,
        honest_label: verified.honest_label(),
    })
}

/// The manifest: the certificate and section that were compiled, the query that was asked, and the
/// world by digest.
///
/// The world is referenced rather than carried, which is the honest shape for it and also the
/// interesting one: verification then reports one entry it could not check, so a slice report
/// cannot be read as "everything verified".
fn assemble(
    probe: &BundleProbe,
    inputs: &BundleInputs<'_>,
) -> Result<ResultBundle, bioprism_bundle::BundleError> {
    let world_digest = ContentHash::of_value(inputs.world_document)?;
    ResultBundle::builder(probe.bundle_id.clone())
        .carrying_certificate(
            CERTIFICATE,
            &inputs.output.certificate,
            bioprism_section::CertificateProfile::Extended,
        )?
        .carrying(
            SECTION,
            EntryRole::DecisionSection,
            inputs.output.section.to_json(),
        )?
        .carrying(QUERY, EntryRole::Query, inputs.query_document.clone())?
        .referencing(
            WORLD,
            EntryRole::World,
            world_digest.clone(),
            Some(format!("worldgen://{}", inputs.output.certificate.world_id)),
            ProvenanceState::Recorded(
                RecordedProvenance::new("bioprism-worldgen", world_digest)
                    .pinned_at(inputs.spec_digest.as_str()),
            ),
        )
        .build()
}

/// Every [`AttestationCheck`] state gets its own word, with no wildcard arm.
///
/// A new outcome in `bioprism-bundle` therefore breaks this crate at compile time instead of
/// arriving as an unnamed default that a slice's expectation would silently accept — the same
/// stance [`crate::slice`] takes on compiler refusals.
fn outcome_word(check: &AttestationCheck) -> &'static str {
    match check {
        AttestationCheck::KeyHolderAuthenticated(_) => "key_holder_authenticated",
        AttestationCheck::WrongKeyOffered { .. } => "wrong_key_offered",
        AttestationCheck::TagMismatch { .. } => "tag_mismatch",
        AttestationCheck::PurposeMismatch { .. } => "purpose_mismatch",
        AttestationCheck::Malformed(_) => "malformed",
    }
}

fn certificate_word(state: EmbeddedCertificate) -> &'static str {
    match state {
        EmbeddedCertificate::SelfVerified => "self_verified",
        EmbeddedCertificate::NotCarried => "not_carried",
        EmbeddedCertificate::Absent => "absent",
    }
}
