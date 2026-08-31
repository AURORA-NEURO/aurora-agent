//! Sources and their manifests.
//!
//! Blueprint 40.17 lists "source locator/credentials, format/profile/version, mapping policy,
//! identity/time context" as adapter inputs and "artifact inventory" plus "reader provenance"
//! as outputs. [`Source`] is the first two; [`SourceManifest`] is the last two.
//!
//! Invariant 1 — "source bytes remain addressable" — is enforced by making every ingestion
//! carry a manifest whose digest is taken over the source itself, not over the facts derived
//! from it. That digest is what lets a reader who distrusts a world go back to the bytes; it
//! is also how 40.17's "source drift" failure mode is detected, by re-probing later and
//! comparing.
//!
//! Credentials are deliberately absent. This crate ingests bytes that the caller has already
//! obtained, so it never holds a token, and a controlled-access source is fetched by whatever
//! layer is authorised to hold that authority. Keeping the fetch out of the adapter is what
//! makes the adapter safe to run on untrusted mapping policies.

use crate::error::AdapterError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where the bytes are.
///
/// Only two shapes exist because only two are honestly supported here: a byte buffer the
/// caller already has, and a directory on the local filesystem. Network locators are not
/// modelled — see the crate docs on why fetching belongs above this layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Locator {
    /// A single artifact held in memory.
    Bytes(Vec<u8>),
    /// A directory to be walked recursively.
    Directory(PathBuf),
}

/// What is known about where a source came from, before any adapter looks at it.
///
/// All three fields are optional and none of them are inferred. An adapter that finds this
/// absent declares a [`crate::LossKind::ProvenanceUnavailable`] rather than fabricating a
/// retrieval time from the filesystem mtime, which would be a claim about the upstream
/// repository that the filesystem cannot support.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProvenance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accession: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// RFC 3339 retrieval time as supplied by the caller. Not parsed here; carried verbatim so
    /// that a malformed upstream value is visible downstream rather than normalized away.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieved_at: Option<String>,
}

impl SourceProvenance {
    pub fn validate(&self) -> Result<(), AdapterError> {
        for (field, value) in [
            ("provenance.accession", self.accession.as_deref()),
            ("provenance.version", self.version.as_deref()),
            ("provenance.retrieved_at", self.retrieved_at.as_deref()),
        ] {
            if let Some(value) = value {
                validate_text(field, value, 512)?;
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.accession.is_none() && self.version.is_none() && self.retrieved_at.is_none()
    }

    /// The provenance strings to attach to every fact derived from this source.
    pub fn strings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(accession) = &self.accession {
            out.push(format!("accession={accession}"));
        }
        if let Some(version) = &self.version {
            out.push(format!("version={version}"));
        }
        if let Some(retrieved_at) = &self.retrieved_at {
            out.push(format!("retrieved_at={retrieved_at}"));
        }
        out
    }
}

/// One thing to ingest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// Caller-chosen identifier. Appears in every [`crate::SourceLocation`] this source
    /// produces and in every emitted fact id, so changing it changes the world.
    pub id: String,
    pub locator: Locator,
    /// The format the caller asserts, e.g. `text/csv`. Never sniffed from the bytes: content
    /// sniffing is exactly the "adapter guesses silently" behaviour 40.17 forbids, and a
    /// mis-sniffed table becomes a world full of plausible, wrong facts.
    pub declared_format: Option<String>,
    pub provenance: Option<SourceProvenance>,
}

impl Source {
    pub fn validate(&self) -> Result<(), AdapterError> {
        validate_text("source_id", &self.id, 512)?;
        if let Some(format) = &self.declared_format {
            validate_text("declared_format", format, 256)?;
            if format != &format.to_ascii_lowercase() {
                return Err(AdapterError::InvalidSource(
                    "declared_format must be lowercase".into(),
                ));
            }
        }
        if let Some(provenance) = &self.provenance {
            provenance.validate()?;
        }
        Ok(())
    }

    pub fn bytes(id: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Source {
            id: id.into(),
            locator: Locator::Bytes(bytes.into()),
            declared_format: None,
            provenance: None,
        }
    }

    pub fn directory(id: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Source {
            id: id.into(),
            locator: Locator::Directory(root.into()),
            declared_format: None,
            provenance: None,
        }
    }

    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.declared_format = Some(format.into());
        self
    }

    pub fn with_provenance(mut self, provenance: SourceProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// The bytes, when this source is a single in-memory artifact.
    pub fn as_bytes(&self, adapter: &'static str) -> Result<&[u8], AdapterError> {
        self.validate()?;
        match &self.locator {
            Locator::Bytes(bytes) => Ok(bytes),
            Locator::Directory(_) => Err(AdapterError::UnsupportedSource {
                adapter,
                source_id: self.id.clone(),
                expected: "an in-memory byte artifact, not a directory",
            }),
        }
    }

    /// The root, when this source is a directory.
    pub fn as_directory(&self, adapter: &'static str) -> Result<&Path, AdapterError> {
        self.validate()?;
        match &self.locator {
            Locator::Directory(root) => Ok(root),
            Locator::Bytes(_) => Err(AdapterError::UnsupportedSource {
                adapter,
                source_id: self.id.clone(),
                expected: "a directory, not an in-memory byte artifact",
            }),
        }
    }

    /// Rejects a declared format the adapter does not implement.
    ///
    /// An absent declaration is accepted: the caller who says nothing gets the adapter's
    /// default handling, which each adapter documents. A caller who declares `application/dicom`
    /// to a CSV adapter gets a typed refusal instead of a table of nonsense.
    pub fn require_format(
        &self,
        adapter: &'static str,
        accepted: &[&str],
    ) -> Result<(), AdapterError> {
        self.validate()?;
        match &self.declared_format {
            None => Ok(()),
            Some(declared) if accepted.contains(&declared.as_str()) => Ok(()),
            Some(declared) => Err(AdapterError::UnsupportedFormat {
                adapter,
                source_id: self.id.clone(),
                declared: declared.clone(),
            }),
        }
    }
}

/// The reader-provenance record that travels with every ingestion.
///
/// This is what makes an ingest replayable by someone who has neither the adapter's source nor
/// the caller's shell history: the source digest identifies the bytes, the adapter name and
/// version identify the transformation, and the profile digest identifies the mapping policy
/// that was in force. 40.17's definition of done requires exactly this — "inspect every input
/// and transformation through stable locators".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceManifest {
    pub source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_format: Option<String>,
    /// Digest of the source. For a byte artifact this is the digest of the bytes; for a
    /// directory it is the digest of the canonical inventory the adapter walked, because
    /// hashing a directory's "contents" is otherwise undefined.
    pub source_digest: ContentHash,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<u64>,
    pub adapter: String,
    pub adapter_version: String,
    /// Digest of the mapping policy, so that two worlds built from the same bytes by the same
    /// adapter under different mappings are distinguishable without diffing their facts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_digest: Option<ContentHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<SourceProvenance>,
}

impl SourceManifest {
    pub fn validate(&self) -> Result<(), AdapterError> {
        validate_text("source_id", &self.source_id, 512)?;
        validate_text("adapter", &self.adapter, 512)?;
        validate_text("adapter_version", &self.adapter_version, 256)?;
        if let Some(format) = &self.declared_format {
            validate_text("declared_format", format, 256)?;
            if format != &format.to_ascii_lowercase() {
                return Err(AdapterError::InvalidSource(
                    "manifest declared_format must be lowercase".into(),
                ));
            }
        }
        if let Some(provenance) = &self.provenance {
            provenance.validate()?;
        }
        Ok(())
    }
}

fn validate_text(field: &str, value: &str, maximum: usize) -> Result<(), AdapterError> {
    if value.is_empty() || value.trim() != value {
        return Err(AdapterError::InvalidSource(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > maximum || value.chars().any(char::is_control) {
        return Err(AdapterError::InvalidSource(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declared_format_the_adapter_does_not_implement_is_refused_not_attempted() {
        let source = Source::bytes("s", b"a,b\n".to_vec()).with_format("application/dicom");
        let error = source.require_format("tabular", &["text/csv"]).unwrap_err();
        assert!(matches!(error, AdapterError::UnsupportedFormat { .. }));
    }

    #[test]
    fn an_undeclared_format_is_accepted_because_the_caller_asserted_nothing() {
        let source = Source::bytes("s", b"a,b\n".to_vec());
        assert!(source.require_format("tabular", &["text/csv"]).is_ok());
    }

    #[test]
    fn a_directory_source_is_refused_by_a_byte_adapter_with_a_typed_error() {
        let source = Source::directory("s", "/nowhere");
        assert!(matches!(
            source.as_bytes("tabular").unwrap_err(),
            AdapterError::UnsupportedSource { .. }
        ));
    }

    #[test]
    fn empty_provenance_contributes_no_strings() {
        assert!(SourceProvenance::default().strings().is_empty());
        assert!(SourceProvenance::default().is_empty());
    }

    #[test]
    fn source_validation_rejects_noncanonical_format_and_provenance() {
        let source = Source::bytes("source", Vec::new()).with_format("TEXT/CSV");
        assert!(matches!(
            source.validate(),
            Err(AdapterError::InvalidSource(_))
        ));
        let source = Source::bytes("source", Vec::new()).with_provenance(SourceProvenance {
            accession: Some(" accession".into()),
            ..Default::default()
        });
        assert!(matches!(
            source.validate(),
            Err(AdapterError::InvalidSource(_))
        ));
    }

    #[test]
    fn manifest_validation_rejects_missing_adapter_identity() {
        let manifest = SourceManifest {
            source_id: "source".into(),
            declared_format: None,
            source_digest: ContentHash::of_bytes(b"source"),
            byte_length: None,
            adapter: "".into(),
            adapter_version: "0.1.0".into(),
            profile_digest: None,
            provenance: None,
        };
        assert!(matches!(
            manifest.validate(),
            Err(AdapterError::InvalidSource(_))
        ));
    }
}
