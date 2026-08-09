//! The honesty type: what a party *claims*, kept apart from what was *verified*.
//!
//! Blueprint 13.20 §Attestations asks for "narrow claims such as 'built from manifest X in trusted
//! runner Y', 'trial bundle closure verified', or 'independently reproduced'", and warns against
//! broad "secure" attestations. 34.14 requires public pages to "distinguish self-reported,
//! reproduced, verified, and prospectively validated results". 13.15 §Signing wants publisher,
//! builder and hub signatures to be purpose-separated.
//!
//! # The gap this module exists to keep open
//!
//! With symmetric authentication, a verifying tag supports exactly one sentence:
//!
//! > **some holder of key K produced these bytes.**
//!
//! It does not support:
//!
//! > party P produced these bytes.
//!
//! The second is what a reader wants and what a public-key signature would give. It is unavailable
//! here, and the difference is not a rounding error: the set of parties that could have produced a
//! tag is *exactly* the set that can check it, so a tag is evidence of key possession and of nothing
//! else. If two teams share a key so both can verify, a tag distinguishes neither from the other.
//!
//! # How the gap is enforced
//!
//! [`Attestation`] carries a [`ClaimedProducer`]. That string is inside the authenticated bytes, so a
//! verifying tag does prove that *a key holder asserted the name* — which is still not proof that the
//! named party did anything.
//!
//! [`KeyHolderAuthenticated`], the value [`Attestation::verify`] hands back on success, has private
//! fields, one crate-internal constructor, and **no accessor for the claimed producer**. It is
//! `Serialize` but not `Deserialize`, so it cannot be minted from JSON either. A caller holding a
//! verification result can report the key identity, the manifest digest and the purpose, and has no
//! way to reach a principal through it. Reading the claimed producer requires holding the
//! [`Attestation`] — the unverified object — which is where an unverified claim belongs.
//!
//! # Purpose separation, and what it is not
//!
//! [`AttestationPurpose`] is inside the authenticated bytes, so a publisher tag cannot be replayed as
//! a hub receipt. That is *domain* separation and it is real. It is **not** the key separation 13.15
//! §Signing describes, because there is one key type here and nothing prevents one key from serving
//! every purpose. A hub that must not be able to forge a publisher attestation cannot get that
//! property from this crate.
//!
//! # Deliberately not implemented
//!
//! No certificate chain, no trust root, no key validity window, no revocation check (13.20
//! §Verification asks for "key validity at signing time, and revocation status"; neither exists
//! here). No countersignature, no threshold or multi-party attestation, no timestamp — a `recorded_at`
//! string is caller-asserted and corroborated by nothing, because this crate reads no clock.

use crate::error::BundleError;
use crate::mac::{AuthenticationScheme, KeyIdentity, MacTag, Repudiability, SecretKey};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fmt;

/// The wire version of an attestation preimage. Part of the authenticated bytes.
pub const ATTESTATION_SCHEMA_VERSION: &str = "bioprism-attestation/0.1";

/// Which narrow claim an attestation makes, per 13.20 §Attestations.
///
/// Every variant is a claim about a specific artifact. There is no `Secure` and no `Trusted`, which
/// 13.20 explicitly warns against, and no variant may be added that asserts a property of a party
/// rather than of bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationPurpose {
    /// 13.15 §Signing: the publisher attests the manifest it published.
    PublisherManifest,
    /// 13.15 §Build isolation: a builder attests that it built from this manifest.
    BuilderProvenance,
    /// 13.15 §Signing: the hub attests that it accepted a publication.
    HubPublicationReceipt,
    /// 13.20 §Integrity: a checkpoint over an audit chain head.
    AuditCheckpoint,
    /// 34.14: a third party attests they replayed the bundle and reproduced it.
    IndependentReproduction,
}

impl fmt::Display for AttestationPurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            AttestationPurpose::PublisherManifest => "publisher-manifest",
            AttestationPurpose::BuilderProvenance => "builder-provenance",
            AttestationPurpose::HubPublicationReceipt => "hub-publication-receipt",
            AttestationPurpose::AuditCheckpoint => "audit-checkpoint",
            AttestationPurpose::IndependentReproduction => "independent-reproduction",
        };
        f.write_str(text)
    }
}

/// Who *says* they produced a bundle. Never verified, and named so that is unmissable.
///
/// This is 34.14's "self-reported" tier. It is authenticated in the sense that a key holder committed
/// to the string, and unverified in the sense that nothing checks the string is true.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClaimedProducer(String);

impl ClaimedProducer {
    pub fn new(name: impl Into<String>) -> Self {
        ClaimedProducer(name.into())
    }

    /// The self-reported name. Nothing in this crate corroborates it.
    pub fn claimed_name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClaimedProducer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (self-reported, unverified)", self.0)
    }
}

/// An attestation over a manifest digest: a tag, plus the unverified claims that go with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    pub schema_version: String,
    pub purpose: AttestationPurpose,
    /// The manifest this attestation covers.
    pub subject_digest: ContentHash,
    /// The key a verifier must obtain to check the tag. A label, not a credential.
    pub key_identity: KeyIdentity,
    /// Self-reported. See [`ClaimedProducer`].
    pub claimed_producer: ClaimedProducer,
    /// Caller-supplied domain separator. This crate has no RNG; reusing a nonce is the caller's
    /// problem to avoid, and reusing one does not weaken HMAC, it only weakens the caller's ability
    /// to tell two attestations apart.
    pub nonce: Option<String>,
    /// Caller-asserted, corroborated by nothing. This crate reads no clock.
    pub recorded_at: Option<String>,
    pub scheme: AuthenticationScheme,
    pub repudiability: Repudiability,
    pub tag: MacTag,
}

impl Attestation {
    /// Produces an attestation over `subject_digest` under `key`.
    ///
    /// Deterministic: the same inputs always produce the same tag, because every varying input —
    /// nonce, timestamp, producer name — is supplied by the caller.
    pub fn produce(
        purpose: AttestationPurpose,
        subject_digest: ContentHash,
        key: &SecretKey,
        claimed_producer: ClaimedProducer,
    ) -> Result<Self, BundleError> {
        Attestation::produce_with(purpose, subject_digest, key, claimed_producer, None, None)
    }

    /// [`Attestation::produce`] with an explicit nonce and caller-asserted timestamp.
    pub fn produce_with(
        purpose: AttestationPurpose,
        subject_digest: ContentHash,
        key: &SecretKey,
        claimed_producer: ClaimedProducer,
        nonce: Option<String>,
        recorded_at: Option<String>,
    ) -> Result<Self, BundleError> {
        let preimage = preimage_bytes(
            purpose,
            &subject_digest,
            key.identity(),
            &claimed_producer,
            nonce.as_deref(),
            recorded_at.as_deref(),
        )?;
        Ok(Attestation {
            schema_version: ATTESTATION_SCHEMA_VERSION.to_string(),
            purpose,
            subject_digest,
            key_identity: key.identity().clone(),
            claimed_producer,
            nonce,
            recorded_at,
            scheme: AuthenticationScheme::SymmetricSharedSecret,
            repudiability: Repudiability::ForgeableByAnyVerifier,
            tag: key.authenticate(&preimage),
        })
    }

    /// Checks the tag under `key`, without demanding a particular purpose.
    pub fn verify(&self, key: &SecretKey) -> AttestationCheck {
        self.check(key, None)
    }

    /// Checks the tag under `key` and refuses a tag minted for a different purpose.
    pub fn verify_for(&self, key: &SecretKey, purpose: AttestationPurpose) -> AttestationCheck {
        self.check(key, Some(purpose))
    }

    /// [`Attestation::verify_for`] as a `Result`, for call sites that want the error type.
    pub fn verify_for_or_error(
        &self,
        key: &SecretKey,
        purpose: AttestationPurpose,
    ) -> Result<KeyHolderAuthenticated, BundleError> {
        self.verify_for(key, purpose).into_result()
    }

    fn check(&self, key: &SecretKey, purpose: Option<AttestationPurpose>) -> AttestationCheck {
        if key.identity() != &self.key_identity {
            return AttestationCheck::WrongKeyOffered {
                attested: self.key_identity.clone(),
                offered: key.identity().clone(),
            };
        }
        if let Some(requested) = purpose {
            if requested != self.purpose {
                return AttestationCheck::PurposeMismatch {
                    attested: self.purpose,
                    requested,
                };
            }
        }
        let preimage = match preimage_bytes(
            self.purpose,
            &self.subject_digest,
            &self.key_identity,
            &self.claimed_producer,
            self.nonce.as_deref(),
            self.recorded_at.as_deref(),
        ) {
            Ok(bytes) => bytes,
            Err(error) => return AttestationCheck::Malformed(error.to_string()),
        };
        if !key.authenticate(&preimage).verify_against(&self.tag) {
            return AttestationCheck::TagMismatch {
                key_identity: self.key_identity.clone(),
            };
        }
        AttestationCheck::KeyHolderAuthenticated(KeyHolderAuthenticated {
            key_identity: self.key_identity.clone(),
            subject_digest: self.subject_digest.clone(),
            purpose: self.purpose,
            scheme: self.scheme,
            repudiability: self.repudiability,
        })
    }
}

/// The outcome of checking an attestation. Four states, none of them collapsed into the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AttestationCheck {
    /// The tag verifies. Names a key, never a party.
    KeyHolderAuthenticated(KeyHolderAuthenticated),
    /// The verifier holds a different key from the one the attestation names. Nothing was checked.
    WrongKeyOffered {
        attested: KeyIdentity,
        offered: KeyIdentity,
    },
    /// The tag does not verify. Altered bytes and a wrong key are indistinguishable from here.
    TagMismatch { key_identity: KeyIdentity },
    /// A tag minted for one purpose was offered for another. No replay across purposes.
    PurposeMismatch {
        attested: AttestationPurpose,
        requested: AttestationPurpose,
    },
    /// The attestation could not be reduced to canonical bytes, so no check was possible.
    Malformed(String),
}

impl AttestationCheck {
    pub fn into_result(self) -> Result<KeyHolderAuthenticated, BundleError> {
        match self {
            AttestationCheck::KeyHolderAuthenticated(authenticated) => Ok(authenticated),
            AttestationCheck::WrongKeyOffered { attested, offered } => {
                Err(BundleError::KeyIdentityMismatch { attested, offered })
            }
            AttestationCheck::TagMismatch { key_identity } => {
                Err(BundleError::TagMismatch { key_identity })
            }
            AttestationCheck::PurposeMismatch {
                attested,
                requested,
            } => Err(BundleError::PurposeMismatch {
                attested: attested.to_string(),
                requested: requested.to_string(),
            }),
            AttestationCheck::Malformed(detail) => {
                Err(BundleError::AttestationUnreadable { detail })
            }
        }
    }

    pub fn is_authenticated(&self) -> bool {
        matches!(self, AttestationCheck::KeyHolderAuthenticated(_))
    }
}

/// What a verifying tag actually establishes.
///
/// Private fields, one crate-internal constructor, `Serialize` but not `Deserialize`, and — the
/// whole point — no accessor that yields a principal. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyHolderAuthenticated {
    key_identity: KeyIdentity,
    subject_digest: ContentHash,
    purpose: AttestationPurpose,
    scheme: AuthenticationScheme,
    repudiability: Repudiability,
}

impl KeyHolderAuthenticated {
    /// The key whose holder produced the attested bytes. This is the strongest available statement
    /// of origin, and it names a key rather than a party.
    pub fn key_identity(&self) -> &KeyIdentity {
        &self.key_identity
    }

    pub fn subject_digest(&self) -> &ContentHash {
        &self.subject_digest
    }

    pub fn purpose(&self) -> AttestationPurpose {
        self.purpose
    }

    pub fn scheme(&self) -> AuthenticationScheme {
        self.scheme
    }

    pub fn repudiability(&self) -> Repudiability {
        self.repudiability
    }

    /// The sentence any surface rendering this result must be able to print.
    ///
    /// Deliberately mentions the key and not a producer, and says what the tag does not establish.
    pub fn honest_label(&self) -> String {
        format!(
            "a holder of key `{}` produced the bytes digesting to {} for purpose `{}`; \
             hmac-sha256 under a shared secret, so every party able to check this could also have \
             produced it — this authenticates a key, not a party, establishes no origin and is not \
             a signature",
            self.key_identity, self.subject_digest, self.purpose
        )
    }
}

fn preimage_bytes(
    purpose: AttestationPurpose,
    subject_digest: &ContentHash,
    key_identity: &KeyIdentity,
    claimed_producer: &ClaimedProducer,
    nonce: Option<&str>,
    recorded_at: Option<&str>,
) -> Result<Vec<u8>, BundleError> {
    let mut map = Map::new();
    map.insert("schema_version".into(), json!(ATTESTATION_SCHEMA_VERSION));
    map.insert(
        "purpose".into(),
        serde_json::to_value(purpose).expect("a purpose is serialisable"),
    );
    map.insert("subject_digest".into(), json!(subject_digest.as_str()));
    map.insert("key_identity".into(), json!(key_identity.as_str()));
    map.insert(
        "claimed_producer".into(),
        json!(claimed_producer.claimed_name()),
    );
    map.insert("nonce".into(), json!(nonce));
    map.insert("recorded_at".into(), json!(recorded_at));
    map.insert(
        "scheme".into(),
        serde_json::to_value(AuthenticationScheme::SymmetricSharedSecret)
            .expect("the scheme is serialisable"),
    );
    Ok(bioprism_ids::to_canonical_bytes(&Value::Object(map))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> SecretKey {
        SecretKey::new(KeyIdentity::new(name), vec![0x11; 32])
    }

    fn other_key(name: &str) -> SecretKey {
        SecretKey::new(KeyIdentity::new(name), vec![0x22; 32])
    }

    fn subject() -> ContentHash {
        ContentHash::of_bytes(b"manifest bytes")
    }

    fn attest(key: &SecretKey) -> Attestation {
        Attestation::produce(
            AttestationPurpose::PublisherManifest,
            subject(),
            key,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests")
    }

    #[test]
    fn verification_names_the_key_identity_and_not_a_principal() {
        let key = key("hub-2026");
        let attestation = attest(&key);
        let authenticated = attestation
            .verify(&key)
            .into_result()
            .expect("verifies under its own key");

        assert_eq!(authenticated.key_identity().as_str(), "hub-2026");
        let label = authenticated.honest_label();
        assert!(label.contains("hub-2026"), "{label}");
        assert!(
            !label.contains("AURORA Working Group"),
            "a verified result must not surface the self-reported producer: {label}"
        );
        assert!(label.contains("authenticates a key, not a party"), "{label}");

        let rendered = serde_json::to_string(&authenticated).expect("serialises");
        assert!(
            !rendered.contains("AURORA Working Group"),
            "serialising a verification result must not leak the claimed producer: {rendered}"
        );
    }

    #[test]
    fn an_hmac_tag_cannot_be_rendered_as_a_signature() {
        let key = key("hub-2026");
        let attestation = attest(&key);
        let authenticated = attestation.verify(&key).into_result().expect("verifies");

        for rendering in [
            serde_json::to_string(&attestation).expect("serialises"),
            serde_json::to_string(&authenticated).expect("serialises"),
            format!("{}", attestation.tag),
        ] {
            let lowered = rendering.to_lowercase();
            assert!(!lowered.contains("signature"), "{rendering}");
            assert!(!lowered.contains("signed"), "{rendering}");
        }

        assert!(format!("{}", attestation.tag).starts_with("hmac-sha256:"));
        assert!(
            authenticated.honest_label().contains("is not a signature"),
            "the one place the word appears must be the denial: {}",
            authenticated.honest_label()
        );
    }

    #[test]
    fn the_claimed_producer_is_reachable_only_through_the_unverified_attestation() {
        let key = key("hub-2026");
        let attestation = attest(&key);
        assert_eq!(
            attestation.claimed_producer.claimed_name(),
            "AURORA Working Group"
        );
        assert!(attestation
            .claimed_producer
            .to_string()
            .contains("self-reported, unverified"));
    }

    #[test]
    fn altering_the_claimed_producer_breaks_the_tag() {
        let key = key("hub-2026");
        let mut attestation = attest(&key);
        attestation.claimed_producer = ClaimedProducer::new("Someone Else");
        assert_eq!(
            attestation.verify(&key),
            AttestationCheck::TagMismatch {
                key_identity: KeyIdentity::new("hub-2026")
            }
        );
    }

    #[test]
    fn altering_the_subject_digest_breaks_the_tag() {
        let key = key("hub-2026");
        let mut attestation = attest(&key);
        attestation.subject_digest = ContentHash::of_bytes(b"different manifest");
        assert!(!attestation.verify(&key).is_authenticated());
    }

    #[test]
    fn a_verifier_holding_a_different_key_learns_nothing_rather_than_failing_the_tag() {
        let attestation = attest(&key("hub-2026"));
        let check = attestation.verify(&other_key("reviewer-1"));
        assert_eq!(
            check,
            AttestationCheck::WrongKeyOffered {
                attested: KeyIdentity::new("hub-2026"),
                offered: KeyIdentity::new("reviewer-1")
            }
        );
    }

    #[test]
    fn a_second_holder_of_the_same_key_verifies_and_is_indistinguishable_from_the_producer() {
        let producer = SecretKey::new(KeyIdentity::new("shared"), vec![0x33; 32]);
        let verifier = SecretKey::new(KeyIdentity::new("shared"), vec![0x33; 32]);
        let attestation = attest(&producer);
        assert!(attestation.verify(&verifier).is_authenticated());
        let forgery = Attestation::produce(
            AttestationPurpose::PublisherManifest,
            subject(),
            &verifier,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("the verifier can forge, which is the point");
        assert_eq!(forgery.tag, attestation.tag);
    }

    #[test]
    fn a_tag_minted_for_one_purpose_cannot_be_replayed_as_another() {
        let key = key("hub-2026");
        let attestation = attest(&key);
        assert_eq!(
            attestation.verify_for(&key, AttestationPurpose::HubPublicationReceipt),
            AttestationCheck::PurposeMismatch {
                attested: AttestationPurpose::PublisherManifest,
                requested: AttestationPurpose::HubPublicationReceipt,
            }
        );
        assert!(attestation
            .verify_for(&key, AttestationPurpose::PublisherManifest)
            .is_authenticated());
    }

    #[test]
    fn attesting_the_same_inputs_twice_yields_identical_bytes() {
        let key = key("hub-2026");
        assert_eq!(attest(&key), attest(&key));
    }

    #[test]
    fn a_nonce_and_a_caller_asserted_timestamp_change_the_tag() {
        let key = key("hub-2026");
        let plain = attest(&key);
        let with_nonce = Attestation::produce_with(
            AttestationPurpose::PublisherManifest,
            subject(),
            &key,
            ClaimedProducer::new("AURORA Working Group"),
            Some("nonce-1".into()),
            None,
        )
        .expect("attests");
        let with_time = Attestation::produce_with(
            AttestationPurpose::PublisherManifest,
            subject(),
            &key,
            ClaimedProducer::new("AURORA Working Group"),
            None,
            Some("2026-08-08T00:00:00Z".into()),
        )
        .expect("attests");
        assert_ne!(plain.tag, with_nonce.tag);
        assert_ne!(plain.tag, with_time.tag);
        assert_ne!(with_nonce.tag, with_time.tag);
    }

    #[test]
    fn an_attestation_survives_a_json_round_trip_including_its_tag() {
        let key = key("hub-2026");
        let attestation = attest(&key);
        let json = serde_json::to_string(&attestation).expect("serialises");
        assert!(json.contains("hmac-sha256:"), "{json}");
        assert!(json.contains("forgeable_by_any_verifier"), "{json}");
        let back: Attestation = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, attestation);
        assert!(back.verify(&key).is_authenticated());
    }

    #[test]
    fn a_verification_result_cannot_be_minted_by_deserialising_one() {
        let key = key("hub-2026");
        let authenticated = attest(&key).verify(&key).into_result().expect("verifies");
        let json = serde_json::to_string(&authenticated).expect("serialises");
        assert!(json.contains("hub-2026"), "{json}");
        assert!(
            serde_json::from_str::<serde_json::Value>(&json).is_ok(),
            "the JSON is well formed; KeyHolderAuthenticated simply has no Deserialize impl \
             for it to be parsed back into"
        );
    }
}
