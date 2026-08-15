//! The repository adapter: what is in a directory, and nothing about what it means.
//!
//! 40.17's first output is an "artifact inventory", and it is the output that makes the rest
//! defensible. Before anything is parsed, a world should be able to say which bytes existed,
//! how many there were, and what their digests are — that is invariant 1, "source bytes remain
//! addressable", and it is what lets a reviewer years later check that the world was built
//! from the archive it claims.
//!
//! This adapter therefore reads bytes and interprets none of them. Every file becomes one fact
//! carrying its path, length and SHA-256, and every file simultaneously becomes one
//! [`LossKind::ContentUninterpreted`] entry, because a digest is a handle on content, not the
//! content. A repository of a thousand DICOM series ingests here as a thousand honest facts
//! and a thousand honest losses, which is a far better starting point than a hundred confident
//! facts derived from a header parser nobody reviewed.
//!
//! **Not implemented here, deliberately:** DICOM, NIfTI/BIDS, AnnData/MuData, OME-Zarr, BAM,
//! CRAM and VCF readers. Those formats are listed as interoperability targets in 28.00 and
//! they belong in the Python layer of 40.14, for two reasons. First, their reference
//! implementations — pydicom, nibabel, anndata, pysam — *are* the specification in practice;
//! a second Rust implementation would be a second opinion about what a file says, and a
//! divergence would surface as biology rather than as a parser bug. Second, this crate has no
//! external dependencies by design, and none of those formats is small enough to hand-roll
//! with the care the CSV reader here received. The right boundary is the one drawn below: Rust
//! establishes what exists and hashes it; a Python adapter that speaks each format converts it
//! and declares its own loss against the same [`crate::SemanticLoss`] contract.

use crate::adapter::{Adapter, AdapterManifest, ConformanceLevel};
use crate::error::AdapterError;
use crate::fact::FactDraft;
use crate::ingestion::Ingestion;
use crate::location::SourceLocation;
use crate::loss::{LossAudit, LossKind, LossSeverity};
use crate::probe::{walk, FileEntry};
use crate::source::{Source, SourceManifest};
use bioprism_ids::ContentHash;
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub const INVENTORY_ADAPTER: &str = "bioprism.inventory";
pub const INVENTORY_ADAPTER_VERSION: &str = "0.1.0";

/// Extensions whose content this crate knowingly cannot read.
///
/// The list exists so that the loss entry for `scan.nii.gz` can say *which* reader is missing
/// rather than the generic "not interpreted" every other file gets. A file not on this list is
/// still uninterpreted; the difference is only in how specific the diagnostic can be.
const MODALITY_EXTENSIONS: [(&str, &str); 13] = [
    ("dcm", "DICOM"),
    ("nii", "NIfTI"),
    ("gz", "possibly gzip-compressed NIfTI or FASTQ"),
    ("h5ad", "AnnData"),
    ("h5mu", "MuData"),
    ("zarr", "OME-Zarr"),
    ("svs", "Aperio whole-slide image"),
    ("bam", "BAM alignment"),
    ("cram", "CRAM alignment"),
    ("vcf", "VCF variants"),
    ("fastq", "FASTQ reads"),
    ("mzml", "mzML spectra"),
    ("bed", "BED intervals"),
];

/// Policy for an inventory walk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryProfile {
    /// Value bound to the `repository` scope dimension of every emitted fact.
    pub repository: String,
    /// Files larger than this are inventoried without a digest.
    ///
    /// The digest is computed over the whole file in memory, so an unbounded walk over a
    /// multi-gigabyte archive would exhaust memory. Skipping the digest is itself a loss and is
    /// declared as one; the alternative — failing the whole ingest — would make the inventory
    /// unavailable precisely for the repositories that most need one.
    pub max_digest_bytes: Option<u64>,
    pub tags: BTreeSet<String>,
}

impl InventoryProfile {
    pub fn new(repository: impl Into<String>) -> Self {
        InventoryProfile {
            repository: repository.into(),
            max_digest_bytes: None,
            tags: BTreeSet::new(),
        }
    }

    pub fn with_max_digest_bytes(mut self, limit: u64) -> Self {
        self.max_digest_bytes = Some(limit);
        self
    }

    pub fn tagged<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    pub fn digest(&self) -> Option<ContentHash> {
        ContentHash::of_value(&serde_json::to_value(self).ok()?).ok()
    }
}

/// Produces one fact per file in a directory tree, and declares every unread byte as loss.
#[derive(Debug, Clone)]
pub struct InventoryAdapter {
    profile: InventoryProfile,
}

impl InventoryAdapter {
    pub fn new(profile: InventoryProfile) -> Self {
        InventoryAdapter { profile }
    }

    pub fn profile(&self) -> &InventoryProfile {
        &self.profile
    }
}

impl Adapter for InventoryAdapter {
    fn name(&self) -> &str {
        INVENTORY_ADAPTER
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new(
            INVENTORY_ADAPTER,
            INVENTORY_ADAPTER_VERSION,
            ConformanceLevel::Normalize,
        )
        .declaring([
            LossKind::ContentUninterpreted,
            LossKind::ProvenanceUnavailable,
        ])
        .binding(["repository", "artifact"])
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        let root = source.as_directory(INVENTORY_ADAPTER)?;
        let entries = walk(root)?;

        let mut audit = LossAudit::new();
        let mut facts = Vec::new();

        let source_provenance = match &source.provenance {
            Some(provenance) if !provenance.is_empty() => provenance.strings(),
            _ => {
                audit.record(
                    LossKind::ProvenanceUnavailable,
                    LossSeverity::Degrading,
                    SourceLocation::source(&source.id),
                    "the caller supplied no upstream accession, version or retrieval time; a \
                     filesystem mtime is not evidence about the upstream repository",
                );
                Vec::new()
            }
        };

        for entry in &entries {
            let location = SourceLocation::artifact(&source.id, &entry.relative);
            audit.mapped(location.clone());

            let mut value = Map::new();
            value.insert("path".to_string(), Value::String(entry.relative.clone()));

            if entry.is_symlink {
                value.insert("symlink".to_string(), Value::Bool(true));
                audit.record(
                    LossKind::ContentUninterpreted,
                    LossSeverity::Degrading,
                    location.clone(),
                    "symbolic link recorded but not followed, so neither its target's bytes \
                     nor its digest are in the world",
                );
            } else {
                let length = entry.length.unwrap_or_default();
                value.insert("byte_length".to_string(), Value::from(length));
                match self.profile.max_digest_bytes {
                    Some(limit) if length > limit => {
                        audit.record(
                            LossKind::ContentUninterpreted,
                            LossSeverity::Blocking,
                            location.clone(),
                            format!(
                                "{length} bytes exceeds the {limit}-byte digest limit, so these \
                                 bytes are named but not addressable by content"
                            ),
                        );
                    }
                    _ => {
                        let bytes = std::fs::read(&entry.absolute)
                            .map_err(|error| AdapterError::io(&entry.relative, &error))?;
                        value.insert(
                            "sha256".to_string(),
                            Value::String(ContentHash::of_bytes(&bytes).to_string()),
                        );
                        audit.record(
                            LossKind::ContentUninterpreted,
                            LossSeverity::Advisory,
                            location.clone(),
                            uninterpreted_detail(entry),
                        );
                    }
                }
            }

            let mut provenance = source_provenance.clone();
            provenance.push(location.locator());

            let scope = ScopeKey::new()
                .exact("repository", &self.profile.repository)
                .exact("artifact", &entry.relative);

            let draft = FactDraft::new(
                format!("fact.artifact.{}.{}", self.profile.repository, entry.relative),
                format!("artifact.{}", entry.relative),
                Value::Object(value),
                scope,
                location,
            )
            .provenances(provenance)
            .tag("artifact")
            .tag("uninterpreted")
            .tags(self.profile.tags.iter().cloned());
            facts.push(draft.build()?);
        }

        let manifest = SourceManifest {
            source_id: source.id.clone(),
            declared_format: source.declared_format.clone(),
            source_digest: inventory_digest(&source.id, &facts)?,
            byte_length: Some(entries.iter().filter_map(|entry| entry.length).sum()),
            adapter: INVENTORY_ADAPTER.to_string(),
            adapter_version: INVENTORY_ADAPTER_VERSION.to_string(),
            profile_digest: self.profile.digest(),
            provenance: source.provenance.clone(),
        };

        Ingestion::new(manifest, facts, audit.finish())
    }
}

/// Digest of the inventory itself, standing in for "the digest of a directory".
///
/// A directory has no bytes, so there is nothing to hash directly. Hashing the canonical list
/// of paths, lengths and per-file digests gives the same property that matters — re-probing
/// later and getting a different value means the source drifted — without pretending that a
/// directory is a file.
fn inventory_digest(source_id: &str, facts: &[Value]) -> Result<ContentHash, AdapterError> {
    let listing = Value::Array(
        facts
            .iter()
            .map(|fact| fact.get("value").cloned().unwrap_or(Value::Null))
            .collect(),
    );
    ContentHash::of_value(&listing)
        .map_err(|source| AdapterError::canonical(SourceLocation::source(source_id), source))
}

fn uninterpreted_detail(entry: &FileEntry) -> String {
    let extension = entry
        .relative
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match MODALITY_EXTENSIONS
        .iter()
        .find(|(suffix, _)| *suffix == extension)
    {
        Some((_, modality)) => format!(
            "bytes hashed but not interpreted: this looks like {modality}, whose reader lives \
             in the Python adapter layer rather than in this crate"
        ),
        None => "bytes hashed but not interpreted; the inventory adapter reads no format".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_modality_extension_names_the_missing_reader() {
        let entry = FileEntry {
            relative: "sub-01/anat/T1w.nii".into(),
            absolute: "T1w.nii".into(),
            length: Some(10),
            is_symlink: false,
        };
        assert!(uninterpreted_detail(&entry).contains("NIfTI"));
    }

    #[test]
    fn a_bed_file_names_the_dependency_free_interval_reader() {
        let entry = FileEntry {
            relative: "regions.bed".into(),
            absolute: "regions.bed".into(),
            length: Some(10),
            is_symlink: false,
        };
        let detail = uninterpreted_detail(&entry);
        assert!(detail.contains("BED intervals"));
        assert!(detail.contains("Python"));
    }

    #[test]
    fn an_unknown_extension_still_declares_the_content_uninterpreted() {
        let entry = FileEntry {
            relative: "notes.txt".into(),
            absolute: "notes.txt".into(),
            length: Some(10),
            is_symlink: false,
        };
        assert!(uninterpreted_detail(&entry).contains("not interpreted"));
    }
}
