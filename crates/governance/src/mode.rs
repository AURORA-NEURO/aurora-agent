//! The declared compatibility mode, and what it does to a field nobody declared.
//!
//! Blueprint 43.35, non-negotiable invariant: *unknown fields are handled according to declared
//! compatibility mode*. The word doing the work is **declared**. A reader that quietly drops what
//! it does not recognise and a writer that assumes its additions survive can interoperate for
//! years and then produce a digest mismatch that looks like a hashing bug. So the mode is a field
//! on [`crate::descriptor::SchemaDescriptor`], both sides state it, and [`negotiate`] turns
//! disagreement into a typed error before any bytes move.
//!
//! # Ignore is a lossy read of a hashed format
//!
//! Dropping an unknown field is harmless for a format nobody hashes. For a format whose identity
//! *is* the hash of its bytes, dropping a field and re-serialising produces a different artifact
//! with the same logical content — exactly 40.37's "old client misreads new object", except that
//! nothing errors. [`UnknownFieldReport::would_alter_digest`] is the detection, and it is why the
//! Context Certificate is declared [`CompatibilityMode::PreserveAndForward`]: that is not a policy
//! preference, it is what `ContextCertificate::verify` already does when it re-hashes every key it
//! did not remove.
//!
//! Not implemented here: any transport. 43.35 speaks of negotiation over CLI, REST, MCP and A2A.
//! This is the decision function those transports would call, with no wire protocol of its own.

use crate::error::CompatibilityError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// What a reader does with a field it does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityMode {
    /// Refuse the document. The strictest posture, and the only one under which an additive change
    /// is genuinely breaking for deployed readers.
    Reject,
    /// Drop the field and carry on. Cheap, and silently destructive for any format whose identity
    /// is a hash of its bytes.
    Ignore,
    /// Keep the field verbatim, including when re-serialising. The only mode under which a
    /// round trip through an older reader leaves a hashed artifact's digest intact.
    PreserveAndForward,
}

impl CompatibilityMode {
    pub fn as_str(self) -> &'static str {
        match self {
            CompatibilityMode::Reject => "reject",
            CompatibilityMode::Ignore => "ignore",
            CompatibilityMode::PreserveAndForward => "preserve-and-forward",
        }
    }

    /// Whether a reader in this mode tolerates a document carrying fields it does not know.
    pub fn tolerates_unknown(self) -> bool {
        !matches!(self, CompatibilityMode::Reject)
    }

    /// Whether a reader in this mode returns the bytes it was given.
    pub fn preserves_bytes(self) -> bool {
        matches!(self, CompatibilityMode::PreserveAndForward)
    }
}

impl fmt::Display for CompatibilityMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The agreed handling for an exchange, once both sides have declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Negotiated {
    pub mode: CompatibilityMode,
}

/// Checks that a writer and a reader declare the same handling.
///
/// There is deliberately no "most conservative wins" fallback. Picking a winner would let the pair
/// proceed while one side's expectations are silently wrong — a writer that believes its additions
/// are forwarded, talking to a reader that drops them, is the precise failure 43.35 names. The
/// remedy is a version negotiation, not a default.
pub fn negotiate(
    writer: CompatibilityMode,
    reader: CompatibilityMode,
) -> Result<Negotiated, CompatibilityError> {
    if writer == reader {
        Ok(Negotiated { mode: reader })
    } else {
        Err(CompatibilityError::ModeDisagreement { writer, reader })
    }
}

/// What happened to the fields a reader did not recognise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnknownFieldReport {
    pub mode: CompatibilityMode,
    /// Paths present in the document and absent from the reader's descriptor.
    pub unknown: Vec<String>,
    /// The subset that the reader dropped.
    pub dropped: Vec<String>,
    /// Whether any dropped field was inside the hashed bytes.
    pub dropped_hashed: bool,
}

impl UnknownFieldReport {
    /// Whether re-serialising what the reader kept would produce a different digest.
    ///
    /// True only when something hashed was actually dropped. An unknown field that the reader
    /// preserved costs nothing, and an unknown field outside the hashed bytes costs nothing
    /// either — the distinction is the reason [`crate::descriptor::DigestRole`] exists.
    pub fn would_alter_digest(&self) -> bool {
        self.dropped_hashed
    }

    pub fn is_clean(&self) -> bool {
        self.dropped.is_empty()
    }
}

/// Applies a reader's declared mode to a document it did not fully recognise.
///
/// `hashed` decides, per unknown path, whether that field sits inside the artifact's canonical
/// bytes. For a whole-document hash — which is what every format in this workspace uses — that
/// predicate is `|_| true`, and the caller passes it explicitly rather than this function assuming
/// either way.
pub fn apply_mode<F>(
    mode: CompatibilityMode,
    document: &Value,
    unknown: &[String],
    hashed: F,
) -> Result<(Value, UnknownFieldReport), CompatibilityError>
where
    F: Fn(&str) -> bool,
{
    match mode {
        CompatibilityMode::Reject => {
            if let Some(first) = unknown.first() {
                return Err(CompatibilityError::UnknownFieldsRejected {
                    count: unknown.len(),
                    first: first.clone(),
                });
            }
            Ok((
                document.clone(),
                UnknownFieldReport {
                    mode,
                    unknown: Vec::new(),
                    dropped: Vec::new(),
                    dropped_hashed: false,
                },
            ))
        }
        CompatibilityMode::Ignore => {
            let mut kept = document.clone();
            let mut dropped = Vec::new();
            let mut dropped_hashed = false;
            for path in unknown {
                if crate::pointer::remove(&mut kept, path).is_some() {
                    dropped_hashed |= hashed(path);
                    dropped.push(path.clone());
                }
            }
            Ok((
                kept,
                UnknownFieldReport {
                    mode,
                    unknown: unknown.to_vec(),
                    dropped,
                    dropped_hashed,
                },
            ))
        }
        CompatibilityMode::PreserveAndForward => Ok((
            document.clone(),
            UnknownFieldReport {
                mode,
                unknown: unknown.to_vec(),
                dropped: Vec::new(),
                dropped_hashed: false,
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_ids::ContentHash;
    use serde_json::json;

    fn document() -> Value {
        json!({"world_id": "w1", "future_field": ["a", "b"]})
    }

    #[test]
    fn a_reader_and_writer_that_declare_different_modes_are_a_typed_error_not_a_default() {
        let error = negotiate(
            CompatibilityMode::PreserveAndForward,
            CompatibilityMode::Ignore,
        )
        .expect_err("disagreement must not be resolved by picking a winner");
        assert!(matches!(
            error,
            CompatibilityError::ModeDisagreement {
                writer: CompatibilityMode::PreserveAndForward,
                reader: CompatibilityMode::Ignore,
            }
        ));
        assert_eq!(
            negotiate(CompatibilityMode::Reject, CompatibilityMode::Reject)
                .expect("agreement negotiates")
                .mode,
            CompatibilityMode::Reject
        );
    }

    #[test]
    fn ignoring_an_unknown_hashed_field_changes_the_digest_and_the_report_says_so() {
        let original = document();
        let (kept, report) = apply_mode(
            CompatibilityMode::Ignore,
            &original,
            &["future_field".to_string()],
            |_| true,
        )
        .expect("ignore never errors");

        assert_eq!(report.dropped, ["future_field"]);
        assert!(report.would_alter_digest());
        let before = ContentHash::of_value(&original).expect("hashes");
        let after = ContentHash::of_value(&kept).expect("hashes");
        assert_ne!(
            before, after,
            "the report claims the digest moved; it must actually move"
        );
    }

    #[test]
    fn preserve_and_forward_leaves_the_digest_of_an_unrecognised_document_intact() {
        let original = document();
        let (kept, report) = apply_mode(
            CompatibilityMode::PreserveAndForward,
            &original,
            &["future_field".to_string()],
            |_| true,
        )
        .expect("preserve never errors");

        assert!(report.is_clean());
        assert!(!report.would_alter_digest());
        assert_eq!(
            ContentHash::of_value(&original).expect("hashes"),
            ContentHash::of_value(&kept).expect("hashes")
        );
    }

    #[test]
    fn ignoring_an_unknown_field_outside_the_hashed_bytes_does_not_claim_a_digest_change() {
        let (_, report) = apply_mode(
            CompatibilityMode::Ignore,
            &document(),
            &["future_field".to_string()],
            |_| false,
        )
        .expect("ignore never errors");
        assert!(!report.is_clean());
        assert!(!report.would_alter_digest());
    }

    #[test]
    fn reject_mode_names_the_first_unknown_field_rather_than_failing_anonymously() {
        let error = apply_mode(
            CompatibilityMode::Reject,
            &document(),
            &["future_field".to_string(), "another".to_string()],
            |_| true,
        )
        .expect_err("reject refuses the document");
        assert!(matches!(
            error,
            CompatibilityError::UnknownFieldsRejected { count: 2, ref first } if first == "future_field"
        ));
    }
}
