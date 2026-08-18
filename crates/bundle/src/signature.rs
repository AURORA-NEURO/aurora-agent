//! Offline Ed25519 attestations for third-party verification.
//!
//! The original bundle path intentionally uses HMAC-SHA256 because it is dependency-light and
//! useful for cooperating processes. HMAC cannot support public verification: anybody who can
//! verify can also forge. This module adds the missing asymmetric boundary without changing the
//! wire meaning of existing [`crate::Attestation`] values.
//!
//! The key registry remains outside this crate. A [`VerificationKey`] proves only that the holder
//! of the corresponding private key signed the canonical bytes. It does not prove that the key
//! identity belongs to the self-reported producer, that the key was authorized for a role, or that
//! the signed timestamp was externally observed. Those are separate policy inputs.

use crate::attestation::{AttestationPurpose, ClaimedProducer, KeyHolderAuthenticated};
use crate::error::BundleError;
use crate::mac::{AuthenticationScheme, KeyIdentity, Repudiability};
use bioprism_ids::ContentHash;
use ed25519_dalek::{Signer, Verifier};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Map, Value};
use std::fmt;

/// The wire version for public-key attestations. It is included in the signed preimage.
pub const PUBLIC_KEY_ATTESTATION_SCHEMA_VERSION: &str = "bioprism-public-key-attestation/0.1";

/// A caller-declared validity window for a verification key.
///
/// Values are Unix seconds, not a local clock reading. Verification compares the supplied
/// `signed_at` value with this window; it never reads the machine clock and never treats an
/// omitted bound as an implicit expiry or activation time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValidity {
    pub not_before: Option<u64>,
    pub not_after: Option<u64>,
}

impl KeyValidity {
    pub fn unbounded() -> Self {
        Self {
            not_before: None,
            not_after: None,
        }
    }

    pub fn bounded(
        not_before: Option<u64>,
        not_after: Option<u64>,
    ) -> Result<Self, SignatureError> {
        if let (Some(before), Some(after)) = (not_before, not_after) {
            if before > after {
                return Err(SignatureError::InvalidValidityWindow {
                    not_before,
                    not_after,
                });
            }
        }
        Ok(Self {
            not_before,
            not_after,
        })
    }

    fn check(&self, signed_at: Option<u64>) -> Result<(), ValidityFailure> {
        if let (Some(not_before), Some(not_after)) = (self.not_before, self.not_after) {
            if not_before > not_after {
                return Err(ValidityFailure::InvalidWindow {
                    not_before,
                    not_after,
                });
            }
        }
        if self.not_before.is_none() && self.not_after.is_none() {
            return Ok(());
        }
        let Some(signed_at) = signed_at else {
            return Err(ValidityFailure::MissingSigningTime);
        };
        if let Some(not_before) = self.not_before {
            if signed_at < not_before {
                return Err(ValidityFailure::BeforeActivation { not_before });
            }
        }
        if let Some(not_after) = self.not_after {
            if signed_at > not_after {
                return Err(ValidityFailure::AfterExpiry { not_after });
            }
        }
        Ok(())
    }
}

/// Ed25519 public key material in an algorithm-labelled wire form.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ed25519PublicKey([u8; 32]);

impl Ed25519PublicKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_prefixed_hex(self) -> String {
        format!("ed25519:{}", hex_lower(&self.0))
    }

    pub fn parse_prefixed_hex(text: &str) -> Result<Self, SignatureError> {
        let Some(hex) = text.strip_prefix("ed25519:") else {
            return Err(SignatureError::MissingAlgorithmPrefix {
                text: text.to_string(),
            });
        };
        Ok(Self(parse_hex::<32>(hex, text)?))
    }
}

impl fmt::Debug for Ed25519PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Ed25519PublicKey")
            .field(&self.to_prefixed_hex())
            .finish()
    }
}

impl fmt::Display for Ed25519PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_prefixed_hex())
    }
}

impl Serialize for Ed25519PublicKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_prefixed_hex())
    }
}

impl<'de> Deserialize<'de> for Ed25519PublicKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse_prefixed_hex(&text).map_err(serde::de::Error::custom)
    }
}

/// An Ed25519 signature in an algorithm-labelled wire form.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ed25519Signature([u8; 64]);

impl Ed25519Signature {
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }

    pub fn to_prefixed_hex(self) -> String {
        format!("ed25519:{}", hex_lower(&self.0))
    }

    pub fn parse_prefixed_hex(text: &str) -> Result<Self, SignatureError> {
        let Some(hex) = text.strip_prefix("ed25519:") else {
            return Err(SignatureError::MissingAlgorithmPrefix {
                text: text.to_string(),
            });
        };
        Ok(Self(parse_hex::<64>(hex, text)?))
    }
}

impl fmt::Debug for Ed25519Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Ed25519Signature")
            .field(&self.to_prefixed_hex())
            .finish()
    }
}

impl fmt::Display for Ed25519Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_prefixed_hex())
    }
}

impl Serialize for Ed25519Signature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_prefixed_hex())
    }
}

impl<'de> Deserialize<'de> for Ed25519Signature {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse_prefixed_hex(&text).map_err(serde::de::Error::custom)
    }
}

/// Private Ed25519 signing material. It is never serializable or printable.
pub struct SigningKey {
    identity: KeyIdentity,
    seed: [u8; 32],
}

impl SigningKey {
    /// Wrap caller-supplied Ed25519 seed bytes. No RNG or key storage is introduced.
    pub fn new(identity: KeyIdentity, seed: [u8; 32]) -> Self {
        Self { identity, seed }
    }

    pub fn identity(&self) -> &KeyIdentity {
        &self.identity
    }

    pub fn verification_key(&self, validity: KeyValidity) -> VerificationKey {
        let signing = ed25519_dalek::SigningKey::from_bytes(&self.seed);
        VerificationKey {
            identity: self.identity.clone(),
            public_key: Ed25519PublicKey::from_bytes(signing.verifying_key().to_bytes()),
            validity,
        }
    }

    fn sign(&self, message: &[u8]) -> Ed25519Signature {
        let signing = ed25519_dalek::SigningKey::from_bytes(&self.seed);
        Ed25519Signature::from_bytes(signing.sign(message).to_bytes())
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKey")
            .field("identity", &self.identity)
            .field("seed", &"<32 bytes redacted>")
            .finish()
    }
}

/// Public verification material and its caller-declared validity window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationKey {
    identity: KeyIdentity,
    public_key: Ed25519PublicKey,
    validity: KeyValidity,
}

impl VerificationKey {
    pub fn new(identity: KeyIdentity, public_key: Ed25519PublicKey, validity: KeyValidity) -> Self {
        Self {
            identity,
            public_key,
            validity,
        }
    }

    pub fn identity(&self) -> &KeyIdentity {
        &self.identity
    }

    pub fn public_key(&self) -> Ed25519PublicKey {
        self.public_key
    }

    pub fn validity(&self) -> &KeyValidity {
        &self.validity
    }

    fn verify(&self, message: &[u8], signature: &Ed25519Signature) -> Result<(), SignatureError> {
        let public = ed25519_dalek::VerifyingKey::from_bytes(self.public_key.as_bytes())
            .map_err(|error| SignatureError::InvalidPublicKey(error.to_string()))?;
        let signature = ed25519_dalek::Signature::from_bytes(signature.as_bytes());
        public
            .verify(message, &signature)
            .map_err(|error| SignatureError::InvalidSignature(error.to_string()))
    }
}

/// A public-key attestation over a manifest or other content digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeyAttestation {
    pub schema_version: String,
    pub purpose: AttestationPurpose,
    pub subject_digest: ContentHash,
    pub key_identity: KeyIdentity,
    pub claimed_producer: ClaimedProducer,
    pub nonce: Option<String>,
    pub recorded_at: Option<String>,
    /// Caller-supplied Unix seconds used only for validity-window evaluation.
    pub signed_at: Option<u64>,
    pub scheme: AuthenticationScheme,
    pub repudiability: Repudiability,
    pub signature: Ed25519Signature,
}

impl PublicKeyAttestation {
    pub fn produce(
        purpose: AttestationPurpose,
        subject_digest: ContentHash,
        key: &SigningKey,
        claimed_producer: ClaimedProducer,
    ) -> Result<Self, BundleError> {
        Self::produce_with(
            purpose,
            subject_digest,
            key,
            claimed_producer,
            None,
            None,
            None,
        )
    }

    pub fn produce_with(
        purpose: AttestationPurpose,
        subject_digest: ContentHash,
        key: &SigningKey,
        claimed_producer: ClaimedProducer,
        nonce: Option<String>,
        recorded_at: Option<String>,
        signed_at: Option<u64>,
    ) -> Result<Self, BundleError> {
        let preimage = preimage_bytes(
            purpose,
            &subject_digest,
            key.identity(),
            &claimed_producer,
            nonce.as_deref(),
            recorded_at.as_deref(),
            signed_at,
        )?;
        Ok(Self {
            schema_version: PUBLIC_KEY_ATTESTATION_SCHEMA_VERSION.to_string(),
            purpose,
            subject_digest,
            key_identity: key.identity().clone(),
            claimed_producer,
            nonce,
            recorded_at,
            signed_at,
            scheme: AuthenticationScheme::Ed25519PublicKey,
            repudiability: Repudiability::NotForgeableByVerifier,
            signature: key.sign(&preimage),
        })
    }

    pub fn verify(&self, key: &VerificationKey) -> PublicKeyAttestationCheck {
        self.check(key, None)
    }

    pub fn verify_for(
        &self,
        key: &VerificationKey,
        purpose: AttestationPurpose,
    ) -> PublicKeyAttestationCheck {
        self.check(key, Some(purpose))
    }

    pub fn verify_for_or_error(
        &self,
        key: &VerificationKey,
        purpose: AttestationPurpose,
    ) -> Result<KeyHolderAuthenticated, BundleError> {
        self.verify_for(key, purpose).into_result()
    }

    fn check(
        &self,
        key: &VerificationKey,
        purpose: Option<AttestationPurpose>,
    ) -> PublicKeyAttestationCheck {
        if self.scheme != AuthenticationScheme::Ed25519PublicKey {
            return PublicKeyAttestationCheck::Malformed(format!(
                "public-key attestation declares scheme `{}`",
                self.scheme
            ));
        }
        if self.schema_version != PUBLIC_KEY_ATTESTATION_SCHEMA_VERSION {
            return PublicKeyAttestationCheck::Malformed(format!(
                "unsupported schema version `{}`",
                self.schema_version
            ));
        }
        if key.identity() != &self.key_identity {
            return PublicKeyAttestationCheck::WrongKeyOffered {
                attested: self.key_identity.clone(),
                offered: key.identity().clone(),
            };
        }
        if let Some(requested) = purpose {
            if requested != self.purpose {
                return PublicKeyAttestationCheck::PurposeMismatch {
                    attested: self.purpose,
                    requested,
                };
            }
        }
        if let Err(failure) = key.validity.check(self.signed_at) {
            return PublicKeyAttestationCheck::KeyNotValidAtSigningTime {
                key_identity: self.key_identity.clone(),
                signed_at: self.signed_at,
                detail: failure.to_string(),
            };
        }
        let preimage = match preimage_bytes(
            self.purpose,
            &self.subject_digest,
            &self.key_identity,
            &self.claimed_producer,
            self.nonce.as_deref(),
            self.recorded_at.as_deref(),
            self.signed_at,
        ) {
            Ok(bytes) => bytes,
            Err(error) => return PublicKeyAttestationCheck::Malformed(error.to_string()),
        };
        match key.verify(&preimage, &self.signature) {
            Ok(()) => PublicKeyAttestationCheck::PublicKeyAuthenticated(
                KeyHolderAuthenticated::public_key_verified(
                    self.key_identity.clone(),
                    self.subject_digest.clone(),
                    self.purpose,
                ),
            ),
            Err(SignatureError::InvalidPublicKey(detail)) => {
                PublicKeyAttestationCheck::Malformed(detail)
            }
            Err(SignatureError::InvalidSignature(_)) => {
                PublicKeyAttestationCheck::SignatureMismatch {
                    key_identity: self.key_identity.clone(),
                }
            }
            Err(error) => PublicKeyAttestationCheck::Malformed(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PublicKeyAttestationCheck {
    PublicKeyAuthenticated(KeyHolderAuthenticated),
    WrongKeyOffered {
        attested: KeyIdentity,
        offered: KeyIdentity,
    },
    SignatureMismatch {
        key_identity: KeyIdentity,
    },
    PurposeMismatch {
        attested: AttestationPurpose,
        requested: AttestationPurpose,
    },
    KeyNotValidAtSigningTime {
        key_identity: KeyIdentity,
        signed_at: Option<u64>,
        detail: String,
    },
    Malformed(String),
}

impl PublicKeyAttestationCheck {
    pub fn into_result(self) -> Result<KeyHolderAuthenticated, BundleError> {
        match self {
            Self::PublicKeyAuthenticated(authenticated) => Ok(authenticated),
            Self::WrongKeyOffered { attested, offered } => {
                Err(BundleError::KeyIdentityMismatch { attested, offered })
            }
            Self::SignatureMismatch { key_identity } => {
                Err(BundleError::SignatureMismatch { key_identity })
            }
            Self::PurposeMismatch {
                attested,
                requested,
            } => Err(BundleError::PurposeMismatch {
                attested: attested.to_string(),
                requested: requested.to_string(),
            }),
            Self::KeyNotValidAtSigningTime {
                key_identity,
                signed_at,
                detail,
            } => {
                if detail == ValidityFailure::MissingSigningTime.to_string() {
                    Err(BundleError::MissingSigningTime { key_identity })
                } else {
                    Err(BundleError::KeyNotValidAtSigningTime {
                        key_identity,
                        signed_at,
                        detail,
                    })
                }
            }
            Self::Malformed(detail) => Err(BundleError::AttestationUnreadable { detail }),
        }
    }

    pub fn is_authenticated(&self) -> bool {
        matches!(self, Self::PublicKeyAuthenticated(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    #[error("`{text}` has no `ed25519:` algorithm prefix")]
    MissingAlgorithmPrefix { text: String },
    #[error("`{text}` is not exactly {expected} lowercase hexadecimal characters")]
    MalformedHex { text: String, expected: usize },
    #[error("validity window not_before={not_before:?} is after not_after={not_after:?}")]
    InvalidValidityWindow {
        not_before: Option<u64>,
        not_after: Option<u64>,
    },
    #[error("invalid Ed25519 public key: {0}")]
    InvalidPublicKey(String),
    #[error("invalid Ed25519 signature: {0}")]
    InvalidSignature(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum ValidityFailure {
    #[error(
        "key validity window is inverted: not_before={not_before} is after not_after={not_after}"
    )]
    InvalidWindow { not_before: u64, not_after: u64 },
    #[error("signing instant was omitted for a bounded key validity window")]
    MissingSigningTime,
    #[error("signing instant is before key activation at {not_before}")]
    BeforeActivation { not_before: u64 },
    #[error("signing instant is after key expiry at {not_after}")]
    AfterExpiry { not_after: u64 },
}

fn preimage_bytes(
    purpose: AttestationPurpose,
    subject_digest: &ContentHash,
    key_identity: &KeyIdentity,
    claimed_producer: &ClaimedProducer,
    nonce: Option<&str>,
    recorded_at: Option<&str>,
    signed_at: Option<u64>,
) -> Result<Vec<u8>, BundleError> {
    let mut map = Map::new();
    map.insert(
        "schema_version".into(),
        json!(PUBLIC_KEY_ATTESTATION_SCHEMA_VERSION),
    );
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
    map.insert("signed_at".into(), json!(signed_at));
    map.insert(
        "scheme".into(),
        serde_json::to_value(AuthenticationScheme::Ed25519PublicKey)
            .expect("the scheme is serialisable"),
    );
    Ok(bioprism_ids::to_canonical_bytes(&Value::Object(map))?)
}

fn parse_hex<const N: usize>(hex: &str, text: &str) -> Result<[u8; N], SignatureError> {
    if hex.len() != N * 2
        || !hex
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(SignatureError::MalformedHex {
            text: text.to_string(),
            expected: N * 2,
        });
    }
    let mut bytes = [0u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let high = hex_value(hex.as_bytes()[index * 2]);
        let low = hex_value(hex.as_bytes()[index * 2 + 1]);
        *byte = (high << 4) | low;
    }
    Ok(bytes)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signing_key() -> SigningKey {
        SigningKey::new(KeyIdentity::new("publisher-ed25519"), [0x42; 32])
    }

    fn attestation() -> PublicKeyAttestation {
        attestation_at(Some(1_755_552_000))
    }

    fn attestation_at(signed_at: Option<u64>) -> PublicKeyAttestation {
        PublicKeyAttestation::produce_with(
            AttestationPurpose::PublisherManifest,
            ContentHash::of_bytes(b"manifest"),
            &signing_key(),
            ClaimedProducer::new("self-reported publisher"),
            Some("nonce-1".into()),
            Some("2026-08-18T20:00:00Z".into()),
            signed_at,
        )
        .expect("attests")
    }

    #[test]
    fn public_key_round_trips_and_third_party_verifies_without_private_material() {
        let signing = signing_key();
        let public = signing.verification_key(KeyValidity::unbounded());
        let attestation = attestation();
        let wire = serde_json::to_string(&attestation).expect("serialises");
        let received: PublicKeyAttestation = serde_json::from_str(&wire).expect("round trips");
        let check = received.verify_for(&public, AttestationPurpose::PublisherManifest);
        assert!(check.is_authenticated(), "{check:?}");
        assert_eq!(public.identity().as_str(), "publisher-ed25519");
        assert!(!wire.contains("0x42"));
    }

    #[test]
    fn tampering_purpose_subject_or_signature_is_rejected() {
        let signing = signing_key();
        let public = signing.verification_key(KeyValidity::unbounded());
        let mut purpose = attestation();
        purpose.purpose = AttestationPurpose::BuilderProvenance;
        assert!(matches!(
            purpose.verify(&public),
            PublicKeyAttestationCheck::SignatureMismatch { .. }
        ));
        let mut subject = attestation();
        subject.subject_digest = ContentHash::of_bytes(b"other");
        assert!(matches!(
            subject.verify(&public),
            PublicKeyAttestationCheck::SignatureMismatch { .. }
        ));
        let mut signature = attestation();
        let mut bytes = *signature.signature.as_bytes();
        bytes[63] ^= 1;
        signature.signature = Ed25519Signature::from_bytes(bytes);
        assert!(matches!(
            signature.verify(&public),
            PublicKeyAttestationCheck::SignatureMismatch { .. }
        ));
    }

    #[test]
    fn purpose_and_identity_are_separate_failures() {
        let signing = signing_key();
        let public = signing.verification_key(KeyValidity::unbounded());
        assert!(matches!(
            attestation().verify_for(&public, AttestationPurpose::HubPublicationReceipt),
            PublicKeyAttestationCheck::PurposeMismatch { .. }
        ));
        let other = VerificationKey::new(
            KeyIdentity::new("other"),
            public.public_key(),
            KeyValidity::unbounded(),
        );
        assert!(matches!(
            attestation().verify(&other),
            PublicKeyAttestationCheck::WrongKeyOffered { .. }
        ));
    }

    #[test]
    fn bounded_key_windows_fail_closed_without_a_clock() {
        let signing = signing_key();
        let public = signing
            .verification_key(KeyValidity::bounded(Some(100), Some(200)).expect("valid window"));
        let missing = attestation_at(None);
        assert!(matches!(
            missing.verify(&public),
            PublicKeyAttestationCheck::KeyNotValidAtSigningTime { .. }
        ));
        let expired = attestation_at(Some(201));
        assert!(matches!(
            expired.verify(&public),
            PublicKeyAttestationCheck::KeyNotValidAtSigningTime { .. }
        ));
        let active = attestation_at(Some(150));
        assert!(active.verify(&public).is_authenticated());
    }

    #[test]
    fn signature_and_key_wire_forms_are_algorithm_labelled() {
        let signing = signing_key();
        let public = signing.verification_key(KeyValidity::unbounded());
        assert!(public
            .public_key()
            .to_prefixed_hex()
            .starts_with("ed25519:"));
        assert!(attestation()
            .signature
            .to_prefixed_hex()
            .starts_with("ed25519:"));
        assert!(Ed25519PublicKey::parse_prefixed_hex("00").is_err());
        assert!(Ed25519Signature::parse_prefixed_hex("hmac-sha256:00").is_err());
    }
}
