//! One canonical form for every IR in this crate.
//!
//! Blueprint 25.01 through 25.21 each repeat the same lifecycle step 4 — "the object receives a
//! canonical serialization and content hash" — and each repeats the same implementation step 4,
//! "add cross-language round-trip tests". Twenty-one modules asking for the same thing is one
//! requirement, not twenty-one, so this crate supplies exactly one implementation of it.
//!
//! The bytes are [`bioprism_ids::to_canonical_bytes`] and the digest is
//! [`bioprism_ids::ContentHash`]. Nothing here re-implements sorting, float formatting or escaping.
//! That matters more than it looks: the workspace's three-way parity (CPython, the eager Rust path,
//! the indexed store) rests on there being exactly one encoder, and a second encoder that agreed
//! today would be a second thing to keep in agreement forever.
//!
//! # What this does not give you
//!
//! A stable digest is not a stable *schema*. Adding a field to an IR changes every digest computed
//! from it, and this crate has no migration machinery — blueprint 25.22 owns schema versioning and
//! is not implemented here. Two digests that differ tell you the values differ; they do not tell
//! you whether the schema moved underneath them.
//!
//! # A finding: the non-finite guard is unreachable through `Serialize`
//!
//! [`bioprism_ids::CanonicalError::NonFiniteNumber`] exists so that a `NaN` tumour volume is refused
//! a digest rather than hashed as a placeholder. It cannot fire on this path. `serde_json`'s
//! `to_value` builds a number with `Number::from_f64`, which returns `None` for any non-finite
//! value, and the serializer then emits `Value::Null`. By the time the canonical encoder sees the
//! tree, the `NaN` has become a JSON `null` and is indistinguishable from a field that was
//! legitimately absent.
//!
//! This is not specific to this crate: **every producer in the workspace that reaches
//! `to_canonical_bytes` through `serde_json::to_value` has the same hole**, and the guard only
//! protects callers that construct a `serde_json::Value` by hand. Two runs that differed only in
//! that one produced `NaN` and the other produced `null` will therefore agree on their digest.
//!
//! Nothing here can close it — a value-level scan after the fact cannot tell the two apart, and
//! there is no offline serializer to swap in. What this crate does instead is refuse the *state*:
//! [`crate::state::BioState::validate`] rejects a resource ledger containing a non-finite amount,
//! so the value never reaches a digest by the normal path. Every other `f64` in the IR family is
//! still exposed to this, and that is stated rather than implied.

use crate::error::IrError;
use bioprism_ids::{to_canonical_bytes, ContentHash};
use serde::Serialize;
use serde_json::Value;

/// Canonical bytes and a content hash, for any IR value in this crate.
///
/// Implemented once, for every `Serialize`, so that no IR can accidentally acquire a private
/// serializer. `serde` attributes on the IR types are therefore the *only* place the wire shape
/// is decided.
pub trait Canonical: Serialize {
    /// The `serde_json` value the canonical encoder runs over.
    fn to_canonical_value(&self) -> Result<Value, IrError> {
        serde_json::to_value(self).map_err(|error| IrError::Encoding {
            subject: std::any::type_name::<Self>().to_string(),
            detail: error.to_string(),
        })
    }

    /// The exact bytes a content hash is taken over.
    fn canonical_bytes(&self) -> Result<Vec<u8>, IrError> {
        let value = self.to_canonical_value()?;
        to_canonical_bytes(&value).map_err(|error| IrError::Encoding {
            subject: std::any::type_name::<Self>().to_string(),
            detail: error.to_string(),
        })
    }

    /// The content hash of this value.
    fn digest(&self) -> Result<ContentHash, IrError> {
        Ok(ContentHash::of_bytes(&self.canonical_bytes()?))
    }
}

impl<T: Serialize> Canonical for T {}

/// Round-trips a value through its canonical form and checks the digest is unchanged.
///
/// The check that actually catches problems is not equality of the values — `serde` will usually
/// give that for free — but equality of the *digests*, because a field that serializes but does not
/// deserialize (a `skip_serializing_if` paired with a missing `default`, most often) silently
/// changes the bytes on the way back.
pub fn round_trips<T>(value: &T) -> Result<bool, IrError>
where
    T: Serialize + serde::de::DeserializeOwned,
{
    let bytes = value.canonical_bytes()?;
    let parsed: T = serde_json::from_slice(&bytes).map_err(|error| IrError::Encoding {
        subject: std::any::type_name::<T>().to_string(),
        detail: error.to_string(),
    })?;
    Ok(parsed.canonical_bytes()? == bytes)
}
