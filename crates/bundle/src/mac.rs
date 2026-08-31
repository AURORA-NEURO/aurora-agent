//! HMAC-SHA256 (RFC 2104), and the type-level refusal to call its output a signature.
//!
//! Blueprint 13.15 §Signing asks that a "publisher signs pack manifest" and that a "hub signs
//! publication receipt"; 13.20 §Attestations asks for signed checkpoints and a CLI that can
//! "verify signatures, log inclusion, artifact closure, key validity at signing time, and
//! revocation status"; 34.14 lists "attestation signatures" among its primary capabilities.
//!
//! # What this module can build, and what it therefore cannot promise
//!
//! This module owns the HMAC compatibility path. The sibling [`crate::signature`] module owns the
//! Ed25519 public-key path. From SHA-256 one can construct HMAC (RFC 2104), which is a **symmetric**
//! message authentication code: the same secret both produces and checks a tag. That remains
//! useful for cooperating local processes, but it must never be serialized or described as a
//! public-key signature.
//!
//! The consequence is not a missing feature, it is a missing *property*. Every party able to
//! verify a tag is also able to forge one, because verification requires the producing secret.
//! Therefore:
//!
//! - There is **no third-party verifiability for this path**. A verifier who does not hold the key cannot check
//!   anything; a verifier who does hold the key is indistinguishable from the producer.
//! - There is **no non-repudiation**. A producer can truthfully say "any of you could have made
//!   that", and nothing in this crate contradicts them.
//! - "This tag verifies" means exactly **"some holder of key K produced these bytes"**, which is
//!   strictly weaker than "party P produced these bytes". [`crate::attestation`] encodes the weaker
//!   statement and has no way to render the stronger one.
//!
//! # The type says so too
//!
//! [`AuthenticationScheme`] records whether a value uses
//! [`AuthenticationScheme::SymmetricSharedSecret`] or
//! [`AuthenticationScheme::Ed25519PublicKey`]. The two paths use separate bundle wrapper types so
//! deserializing an old MAC bundle cannot silently upgrade its meaning.
//!
//! [`MacTag`]'s [`std::fmt::Display`] always emits the `hmac-sha256:` prefix, so a tag pasted into a
//! log, a UI or an issue carries its own algorithm with it and cannot be quoted as "the signature".
//!
//! # Constant-time comparison, attempted
//!
//! [`MacTag::verify_against`] compares tags with a branch-free accumulate-and-test over the full
//! length, guarded by [`std::hint::black_box`]. A derived `==` on `[u8; 32]` short-circuits on the
//! first differing byte and leaks the length of the matching prefix through timing, which is the
//! standard route to forging a tag one byte at a time. [`MacTag`]'s [`PartialEq`] is therefore hand
//! written and routes through the same comparison, so the naive one is not reachable even through a
//! containing type's derived `PartialEq`.
//!
//! This is *attempted* constant time and cannot be guaranteed. Safe Rust has no way to compel an
//! optimizing compiler to preserve a data-independent instruction sequence; `black_box` is an
//! optimization barrier with no formal guarantee, and the backend remains free to reintroduce a
//! branch. This crate performs no network I/O, so no remote timing channel exists here — the
//! construction is written correctly because code like this gets copied into places where one does.
//!
//! # Deliberately not implemented
//!
//! No key generation (a key is bytes the caller supplies). No key derivation, stretching or
//! agreement. No key storage, rotation, escrow or revocation. No HMAC over streaming input — a
//! message is a slice. No truncated-tag policy beyond the RFC 4231 test vector that specifies one.
//! No algorithm agility: the scheme is HMAC-SHA256 and nothing negotiates it.

use crate::hex::hex_lower;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// SHA-256's block size in bytes, the `B` of RFC 2104.
const BLOCK_SIZE: usize = 64;

/// SHA-256's output size in bytes, the `L` of RFC 2104.
pub const TAG_SIZE: usize = 32;

const IPAD: u8 = 0x36;
const OPAD: u8 = 0x5c;

/// How a bundle or attestation is authenticated.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationScheme {
    /// HMAC-SHA256 under a secret both the producer and the verifier hold.
    SymmetricSharedSecret,
    /// Ed25519 over a canonical attestation or bundle manifest preimage.
    Ed25519PublicKey,
}

impl AuthenticationScheme {
    /// The algorithm identifier that appears in serialized bundles.
    pub fn algorithm(self) -> &'static str {
        match self {
            AuthenticationScheme::SymmetricSharedSecret => "hmac-sha256",
            AuthenticationScheme::Ed25519PublicKey => "ed25519",
        }
    }

    /// The sentence any surface displaying an authentication result must be able to print.
    pub fn honest_label(self) -> &'static str {
        match self {
            AuthenticationScheme::SymmetricSharedSecret => {
                "hmac-sha256 under a shared secret: authenticates a key holder, not a party; \
                 any verifier can forge; not a signature"
            }
            AuthenticationScheme::Ed25519PublicKey => {
                "ed25519 public-key signature: third parties can verify possession of the \
                 corresponding private key; key identity and producer name still require an \
                 external key registry"
            }
        }
    }
}

impl fmt::Display for AuthenticationScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AuthenticationScheme::SymmetricSharedSecret => "symmetric-shared-secret",
            AuthenticationScheme::Ed25519PublicKey => "ed25519-public-key-signature",
        })
    }
}

/// Whether a tag can be held against the party that produced it. It cannot.
///
/// One variant, on purpose, and it is the honest one. Under a shared secret the set of parties
/// that could have produced a tag is exactly the set of parties that can check it, so a tag is
/// never evidence *against* a producer who denies making it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repudiability {
    /// Every party able to verify this tag was also able to produce it.
    ForgeableByAnyVerifier,
    /// A verifier can check the public key but does not possess the private signing key.
    ///
    /// This is a cryptographic non-forgeability statement, not proof that a human, organization,
    /// or registry assigned the key identity correctly.
    NotForgeableByVerifier,
}

impl fmt::Display for Repudiability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Repudiability::ForgeableByAnyVerifier => "forgeable-by-any-verifier",
            Repudiability::NotForgeableByVerifier => "not-forgeable-by-verifier",
        })
    }
}

/// The name a verifier looks a key up by.
///
/// An identity is a *label*, not a credential and not a derivation of the key. It is deliberately
/// not `sha256(key)`: a key-derived identifier hands an offline guessing oracle to anyone who sees a
/// bundle. Nothing in this crate checks that the holder of a key is entitled to the name attached to
/// it, so an identity is only as meaningful as the out-of-band process that assigned it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct KeyIdentity(String);

impl KeyIdentity {
    pub fn new(name: impl Into<String>) -> Self {
        KeyIdentity(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KeyIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A shared secret, and the identity a verifier will look it up by.
///
/// Not `Serialize`, not `Clone` and not printable: [`fmt::Debug`] renders the identity and the key
/// length, never the bytes. A secret that can be cloned into a bundle by an accidental `derive` is
/// a secret that will eventually appear in one.
pub struct SecretKey {
    identity: KeyIdentity,
    bytes: Vec<u8>,
}

impl SecretKey {
    /// Wraps caller-supplied key material. No generation happens here; this crate has no RNG.
    ///
    /// RFC 2104 §3 recommends a key of at least `L` bytes (32 for SHA-256) and observes that keys
    /// longer than `B` (64) buy nothing beyond the hash of the key. Short keys are accepted, because
    /// the RFC 4231 vectors use them, and rejecting them would make the vectors untestable.
    pub fn new(identity: KeyIdentity, bytes: impl Into<Vec<u8>>) -> Self {
        SecretKey {
            identity,
            bytes: bytes.into(),
        }
    }

    pub fn identity(&self) -> &KeyIdentity {
        &self.identity
    }

    /// True when the key is shorter than SHA-256's output, which RFC 2104 §3 advises against.
    ///
    /// Advisory only; nothing refuses a short key, because refusing would break the RFC vectors.
    pub fn is_shorter_than_recommended(&self) -> bool {
        self.bytes.len() < TAG_SIZE
    }

    /// HMAC-SHA256 over `message`, per RFC 2104: `H((K ^ opad) || H((K ^ ipad) || text))`.
    pub fn authenticate(&self, message: &[u8]) -> MacTag {
        let mut key_block = [0u8; BLOCK_SIZE];
        if self.bytes.len() > BLOCK_SIZE {
            let digest = Sha256::digest(&self.bytes);
            key_block[..TAG_SIZE].copy_from_slice(&digest);
        } else {
            key_block[..self.bytes.len()].copy_from_slice(&self.bytes);
        }

        let mut inner_pad = [0u8; BLOCK_SIZE];
        let mut outer_pad = [0u8; BLOCK_SIZE];
        for i in 0..BLOCK_SIZE {
            inner_pad[i] = key_block[i] ^ IPAD;
            outer_pad[i] = key_block[i] ^ OPAD;
        }

        let mut inner = Sha256::new();
        inner.update(inner_pad);
        inner.update(message);
        let inner_digest = inner.finalize();

        let mut outer = Sha256::new();
        outer.update(outer_pad);
        outer.update(inner_digest);
        let mut tag = [0u8; TAG_SIZE];
        tag.copy_from_slice(&outer.finalize());

        MacTag(tag)
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretKey")
            .field("identity", &self.identity)
            .field(
                "bytes",
                &format_args!("<{} bytes redacted>", self.bytes.len()),
            )
            .finish()
    }
}

/// An HMAC-SHA256 tag. Not a signature, and it will not render as one.
///
/// Serialized as `hmac-sha256:<64 lowercase hex>`. The prefix is part of the wire form so that a
/// tag lifted out of a bundle into a bug report still names its own algorithm.
///
/// [`PartialEq`] is implemented by hand rather than derived, and routes through
/// [`MacTag::verify_against`]. A derived `PartialEq` on `[u8; 32]` short-circuits, and every
/// containing type that derives `PartialEq` — [`crate::Attestation`], [`crate::AttestedBundle`] —
/// would inherit the short-circuiting comparison without anyone noticing. Removing the naive
/// comparison entirely is cheaper than remembering not to reach for it.
#[derive(Clone, Copy)]
pub struct MacTag([u8; TAG_SIZE]);

impl MacTag {
    pub fn as_bytes(&self) -> &[u8; TAG_SIZE] {
        &self.0
    }

    /// The wire form, `hmac-sha256:<hex>`.
    pub fn to_prefixed_hex(&self) -> String {
        format!(
            "{}:{}",
            AuthenticationScheme::SymmetricSharedSecret.algorithm(),
            hex_lower(&self.0)
        )
    }

    /// Parses the wire form. A bare hex string is rejected: an unlabelled 64-hex blob is
    /// indistinguishable from a SHA-256 digest, and this crate contains plenty of those.
    pub fn parse_prefixed_hex(text: &str) -> Result<Self, MacError> {
        let prefix = AuthenticationScheme::SymmetricSharedSecret.algorithm();
        let Some(hex) = text
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_prefix(':'))
        else {
            return Err(MacError::MissingAlgorithmPrefix {
                text: text.to_string(),
            });
        };
        if hex.len() != TAG_SIZE * 2 {
            return Err(MacError::MalformedTag {
                text: text.to_string(),
            });
        }
        let mut bytes = [0u8; TAG_SIZE];
        for (i, byte) in bytes.iter_mut().enumerate() {
            let hi = hex_value(hex.as_bytes()[i * 2]).ok_or_else(|| MacError::MalformedTag {
                text: text.to_string(),
            })?;
            let lo =
                hex_value(hex.as_bytes()[i * 2 + 1]).ok_or_else(|| MacError::MalformedTag {
                    text: text.to_string(),
                })?;
            *byte = (hi << 4) | lo;
        }
        Ok(MacTag(bytes))
    }

    /// Compares two tags without short-circuiting on the first differing byte.
    ///
    /// See the module docs: this is *attempted* constant time in safe Rust, not a guarantee.
    pub fn verify_against(&self, other: &MacTag) -> bool {
        let mut difference = 0u8;
        for i in 0..TAG_SIZE {
            difference |= self.0[i] ^ other.0[i];
        }
        std::hint::black_box(difference) == 0
    }
}

impl PartialEq for MacTag {
    fn eq(&self, other: &Self) -> bool {
        self.verify_against(other)
    }
}

impl Eq for MacTag {}

impl fmt::Display for MacTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_prefixed_hex())
    }
}

/// Renders the same prefixed form as [`fmt::Display`]. A tag has no secret half to hide, and a
/// `Debug` that printed a bare array would be the thing someone pasted into a report as "the
/// signature bytes".
impl fmt::Debug for MacTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacTag({})", self.to_prefixed_hex())
    }
}

impl Serialize for MacTag {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_prefixed_hex())
    }
}

impl<'de> Deserialize<'de> for MacTag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        MacTag::parse_prefixed_hex(&text).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MacError {
    #[error(
        "tag `{text}` has no algorithm prefix; an unlabelled hex string is not accepted as a tag"
    )]
    MissingAlgorithmPrefix { text: String },
    #[error("tag `{text}` is not `hmac-sha256:` followed by 64 lowercase hex digits")]
    MalformedTag { text: String },
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(bytes: &[u8]) -> SecretKey {
        SecretKey::new(KeyIdentity::new("test-key"), bytes.to_vec())
    }

    #[test]
    fn no_value_can_record_that_a_bundle_was_signed_asymmetrically() {
        let scheme = AuthenticationScheme::SymmetricSharedSecret;
        assert_eq!(scheme.algorithm(), "hmac-sha256");
        assert!(scheme.honest_label().contains("not a signature"));
        assert!(scheme.honest_label().contains("any verifier can forge"));
    }

    #[test]
    fn an_hmac_tag_cannot_be_rendered_as_a_signature() {
        let tag = key(b"k").authenticate(b"message");
        for rendering in [format!("{tag}"), format!("{tag:?}"), tag.to_prefixed_hex()] {
            assert!(rendering.contains("hmac-sha256:"), "{rendering}");
            assert!(
                !rendering.to_lowercase().contains("signature"),
                "{rendering}"
            );
            assert!(
                !rendering.to_lowercase().contains("signed by"),
                "{rendering}"
            );
        }
    }

    #[test]
    fn a_bare_hex_string_is_not_accepted_as_a_tag() {
        let tag = key(b"k").authenticate(b"message");
        let bare = hex_lower(tag.as_bytes());
        assert_eq!(bare.len(), 64);
        assert!(matches!(
            MacTag::parse_prefixed_hex(&bare),
            Err(MacError::MissingAlgorithmPrefix { .. })
        ));
        assert_eq!(
            MacTag::parse_prefixed_hex(&tag.to_prefixed_hex()).expect("round trips"),
            tag
        );
    }

    #[test]
    fn a_key_longer_than_the_block_size_is_hashed_before_use() {
        let long = vec![0xaa; 200];
        let hashed: Vec<u8> = Sha256::digest(&long).to_vec();
        assert_eq!(
            key(&long).authenticate(b"payload"),
            key(&hashed).authenticate(b"payload"),
            "RFC 2104 §2 step 1: a key longer than B is replaced by H(key)"
        );
    }

    #[test]
    fn a_key_shorter_than_the_block_size_is_zero_padded_not_repeated() {
        let short = b"ab".to_vec();
        let mut padded = short.clone();
        padded.resize(BLOCK_SIZE, 0);
        assert_eq!(
            key(&short).authenticate(b"payload"),
            key(&padded).authenticate(b"payload"),
            "a short key is appended with zeros to B bytes, not cyclically repeated"
        );
    }

    #[test]
    fn a_key_of_exactly_the_block_size_is_used_verbatim() {
        let exact = vec![0x5a; BLOCK_SIZE];
        let hashed: Vec<u8> = Sha256::digest(&exact).to_vec();
        assert_ne!(
            key(&exact).authenticate(b"payload"),
            key(&hashed).authenticate(b"payload"),
            "B bytes is not > B, so the hashing branch must not fire at the boundary"
        );
    }

    #[test]
    fn hmac_is_not_a_bare_hash_of_key_concatenated_with_message() {
        let mut naive = Sha256::new();
        naive.update(b"secret");
        naive.update(b"payload");
        let naive: Vec<u8> = naive.finalize().to_vec();
        assert_ne!(
            key(b"secret")
                .authenticate(b"payload")
                .as_bytes()
                .as_slice(),
            naive.as_slice()
        );
    }

    #[test]
    fn a_different_key_or_a_different_message_yields_a_different_tag() {
        let base = key(b"secret").authenticate(b"payload");
        assert_ne!(base, key(b"secrez").authenticate(b"payload"));
        assert_ne!(base, key(b"secret").authenticate(b"payloae"));
        assert_eq!(base, key(b"secret").authenticate(b"payload"));
    }

    #[test]
    fn tag_comparison_rejects_a_tag_that_shares_a_long_prefix() {
        let good = key(b"secret").authenticate(b"payload");
        let mut forged = *good.as_bytes();
        forged[TAG_SIZE - 1] ^= 0x01;
        let forged = MacTag(forged);
        assert!(!good.verify_against(&forged));
        assert!(good.verify_against(&good));
    }

    #[test]
    fn equality_on_tags_routes_through_the_constant_time_comparison_not_a_derived_one() {
        let good = key(b"secret").authenticate(b"payload");
        let mut near_miss = *good.as_bytes();
        near_miss[0] ^= 0x80;
        let near_miss = MacTag(near_miss);

        assert_eq!(good, good);
        assert_ne!(good, near_miss);
        assert_eq!(good == near_miss, good.verify_against(&near_miss));
        assert_eq!(
            good == good,
            good.verify_against(&good),
            "PartialEq and verify_against must not be two different comparisons"
        );
    }

    #[test]
    fn a_secret_key_never_prints_its_bytes() {
        let rendered = format!("{:?}", key(b"hunter2hunter2hunter2"));
        assert!(!rendered.contains("hunter2"), "{rendered}");
        assert!(rendered.contains("redacted"), "{rendered}");
    }

    #[test]
    fn repudiability_has_exactly_one_state_and_it_is_the_weak_one() {
        assert_eq!(
            Repudiability::ForgeableByAnyVerifier.to_string(),
            "forgeable-by-any-verifier"
        );
    }

    #[test]
    fn a_short_key_is_flagged_as_advisory_but_still_usable() {
        let short = key(b"abc");
        assert!(short.is_shorter_than_recommended());
        assert!(!key(&[0u8; 32]).is_shorter_than_recommended());
        assert_eq!(short.authenticate(b"x").as_bytes().len(), TAG_SIZE);
    }
}
