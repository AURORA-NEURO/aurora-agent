//! Content addressing over canonical bytes.

use crate::canonical::{to_canonical_bytes, to_canonical_bytes_serializable};
use crate::error::{CanonicalError, IdError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentHash(String);

impl ContentHash {
    /// Hash a JSON value that has already been materialized.
    ///
    /// This cannot identify floats that a prior serializer converted to `null`; prefer
    /// [`ContentHash::of_serializable`] while the typed source value is still available.
    pub fn of_value(value: &Value) -> Result<Self, CanonicalError> {
        Ok(Self::of_bytes(&to_canonical_bytes(value)?))
    }

    /// Hash serializable Rust data without allowing non-finite floats to become JSON nulls.
    ///
    /// Use this before converting a typed value to `serde_json::Value`; that conversion loses
    /// whether a null originated as a stated null or as NaN/infinity.
    pub fn of_serializable<T: Serialize + ?Sized>(value: &T) -> Result<Self, CanonicalError> {
        Ok(Self::of_bytes(&to_canonical_bytes_serializable(value)?))
    }

    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        ContentHash(hex_lower(&hasher.finalize()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        let well_formed = value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if well_formed {
            Ok(ContentHash(value))
        } else {
            Err(IdError::MalformedContentHash(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<ContentHash> for String {
    fn from(value: ContentHash) -> Self {
        value.0
    }
}

impl TryFrom<String> for ContentHash {
    type Error = IdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ContentHash::parse(value)
    }
}

pub fn sha256_hex_of_value(value: &Value) -> Result<String, CanonicalError> {
    ContentHash::of_value(value).map(|h| h.0)
}

pub fn sha256_hex_of_serializable<T: Serialize + ?Sized>(
    value: &T,
) -> Result<String, CanonicalError> {
    ContentHash::of_serializable(value).map(|hash| hash.0)
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
