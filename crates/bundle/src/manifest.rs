//! The manifest: the list of digests a tag actually covers.
//!
//! Blueprint 34.14 requires a result card to resolve to "configuration, worlds, runs, oracles, and
//! statistical code", and its API object carries `provenance` and `links` to worlds, cells, oracles
//! and results. 13.15 §Signing has the publisher sign "the pack manifest" rather than each artifact,
//! which only works if the manifest binds every artifact by digest — otherwise a tag over the
//! manifest authenticates a list of names.
//!
//! # Why a tag over the manifest is enough
//!
//! Every entry records a [`ContentHash`], so the manifest's own canonical bytes transitively cover
//! the content of every entry. One tag over the manifest therefore covers the whole closure, and a
//! single altered byte anywhere in the carried content changes an entry digest, changes the manifest
//! bytes and breaks the tag.
//!
//! That chain has one link that is not cryptographic: it holds only if verification *recomputes* the
//! entry digests from the carried content instead of reading them out of the manifest.
//! [`crate::bundle::ResultBundle::verify`] recomputes, and [`crate::error::BundleError::EntryDigestMismatch`]
//! names the entry when the recomputation disagrees.
//!
//! # Inline and referenced entries
//!
//! A world can be far larger than a result. [`EntryBody::Reference`] records an entry's digest
//! without carrying its content, which keeps the entry inside the authenticated closure while
//! leaving the bytes elsewhere. The cost is stated rather than hidden: a referenced entry cannot be
//! recomputed at verification time, so it is reported as [`crate::bundle::EntryCheck::NotCarried`]
//! and never as a passing check.
//!
//! # Deliberately not implemented
//!
//! No dereferencing of a reference locator — this crate performs no I/O. No entry compression, no
//! chunking, no Merkle tree over entries (a flat sorted list is enough at this size and a tree would
//! imply inclusion proofs this crate cannot verify against anything external). No schema migration:
//! [`BUNDLE_SCHEMA_VERSION`] is inside the hashed bytes, so a version change changes every digest,
//! which is the intent.

use crate::environment::{EnvironmentFacts, ToolchainFacts};
use crate::error::BundleError;
use crate::mac::{AuthenticationScheme, Repudiability};
use crate::provenance::{ProvenanceState, SupplyChainPosture};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The wire version of a bundle manifest. Part of the hashed bytes.
pub const BUNDLE_SCHEMA_VERSION: &str = "bioprism-result-bundle/0.1";

/// What an entry is, in the vocabulary 34.14 uses for a result card.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryRole {
    /// The 43.26 Context Certificate. Exactly one per bundle.
    ContextCertificate,
    /// The 43.25 Decision Section the certificate describes. Exactly one per bundle.
    DecisionSection,
    /// The world the section was compiled from, usually by reference.
    World,
    /// The decision query.
    Query,
    /// An oracle or reference standard consulted.
    Oracle,
    /// A benchmark pack, per 13.15's pack manifest.
    BenchmarkPack,
    /// Anything else a reproduction needs, named by the caller.
    Auxiliary { label: String },
}

impl fmt::Display for EntryRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryRole::ContextCertificate => f.write_str("context_certificate"),
            EntryRole::DecisionSection => f.write_str("decision_section"),
            EntryRole::World => f.write_str("world"),
            EntryRole::Query => f.write_str("query"),
            EntryRole::Oracle => f.write_str("oracle"),
            EntryRole::BenchmarkPack => f.write_str("benchmark_pack"),
            EntryRole::Auxiliary { label } => write!(f, "auxiliary:{label}"),
        }
    }
}

/// Whether the bundle carries an entry's content or only its digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "body", rename_all = "snake_case")]
pub enum EntryBody {
    /// Content travels with the bundle and its digest is recomputed at verification time.
    Inline,
    /// Only the digest travels. The locator, if present, is opaque and never dereferenced here.
    Reference { locator: Option<String> },
}

impl EntryBody {
    pub fn is_inline(&self) -> bool {
        matches!(self, EntryBody::Inline)
    }
}

/// One row of the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// The key content is indexed by. Unique within a manifest; duplicates are
    /// [`BundleError::DuplicateEntry`].
    pub name: String,
    pub role: EntryRole,
    /// The claim. Verification recomputes this rather than trusting it.
    pub digest: ContentHash,
    pub body: EntryBody,
    /// 13.15: where this input came from, or the fact that nobody recorded it.
    pub provenance: ProvenanceState,
}

impl ManifestEntry {
    pub fn new(name: impl Into<String>, role: EntryRole, digest: ContentHash) -> Self {
        ManifestEntry {
            name: name.into(),
            role,
            digest,
            body: EntryBody::Inline,
            provenance: ProvenanceState::Unrecorded,
        }
    }

    pub fn referenced(mut self, locator: Option<String>) -> Self {
        self.body = EntryBody::Reference { locator };
        self
    }

    pub fn with_provenance(mut self, provenance: ProvenanceState) -> Self {
        self.provenance = provenance;
        self
    }
}

/// The authenticated part of a bundle.
///
/// Entries are kept sorted by name so that the canonical bytes depend on the set of entries and not
/// on the order a caller happened to add them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub schema_version: String,
    /// A caller-supplied identifier. Not a digest, not unique by construction, and not checked.
    pub bundle_id: String,
    pub entries: Vec<ManifestEntry>,
    pub environment: EnvironmentFacts,
    pub toolchain: ToolchainFacts,
    /// Recorded inside the hashed bytes so a reader of a stored bundle learns the crate's
    /// limitation from the bundle rather than from this documentation.
    pub scheme: AuthenticationScheme,
    pub repudiability: Repudiability,
}

impl BundleManifest {
    /// Builds a manifest, sorting entries and rejecting duplicate names.
    pub fn new(
        bundle_id: impl Into<String>,
        mut entries: Vec<ManifestEntry>,
        environment: EnvironmentFacts,
        toolchain: ToolchainFacts,
    ) -> Result<Self, BundleError> {
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        for pair in entries.windows(2) {
            if pair[0].name == pair[1].name {
                return Err(BundleError::DuplicateEntry {
                    entry: pair[0].name.clone(),
                });
            }
        }
        Ok(BundleManifest {
            schema_version: BUNDLE_SCHEMA_VERSION.to_string(),
            bundle_id: bundle_id.into(),
            entries,
            environment,
            toolchain,
            scheme: AuthenticationScheme::SymmetricSharedSecret,
            repudiability: Repudiability::ForgeableByAnyVerifier,
        })
    }

    pub fn entry(&self, name: &str) -> Option<&ManifestEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    /// The single entry with a given role, or a cardinality error naming the role.
    ///
    /// 34.14 requires every rendered score to resolve to immutable result objects, which needs
    /// exactly one certificate and one section per bundle rather than a best-effort first match.
    pub fn sole_entry_with_role(&self, role: &EntryRole) -> Result<&ManifestEntry, BundleError> {
        let matches: Vec<&ManifestEntry> =
            self.entries.iter().filter(|entry| &entry.role == role).collect();
        match matches.as_slice() {
            [only] => Ok(only),
            other => Err(BundleError::RoleCardinality {
                role: role.to_string(),
                found: other.len(),
            }),
        }
    }

    /// The canonical bytes a tag is computed over. Uses `bioprism-ids`' canonical serializer, the
    /// workspace's only one, so bundle bytes and certificate bytes agree on float and key ordering.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, BundleError> {
        let value = serde_json::to_value(self).expect("a manifest is serialisable");
        Ok(bioprism_ids::to_canonical_bytes(&value)?)
    }

    pub fn digest(&self) -> Result<ContentHash, BundleError> {
        Ok(ContentHash::of_bytes(&self.canonical_bytes()?))
    }

    /// 13.15's three-state picture across every input, with the states kept apart.
    pub fn supply_chain_posture(&self) -> SupplyChainPosture {
        let mut posture = SupplyChainPosture::default();
        for entry in &self.entries {
            match &entry.provenance {
                ProvenanceState::Recorded(_) => posture.recorded.push(entry.name.clone()),
                ProvenanceState::Unrecorded => posture.unrecorded.push(entry.name.clone()),
                ProvenanceState::Rejected(_) => posture.rejected.push(entry.name.clone()),
            }
        }
        posture
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn manifest(entries: Vec<ManifestEntry>) -> Result<BundleManifest, BundleError> {
        BundleManifest::new(
            "run-1",
            entries,
            EnvironmentFacts::undeclared(),
            ToolchainFacts::declared(),
        )
    }

    #[test]
    fn two_entries_with_the_same_name_are_refused_rather_than_silently_deduplicated() {
        let error = manifest(vec![
            ManifestEntry::new("certificate", EntryRole::ContextCertificate, digest("a")),
            ManifestEntry::new("certificate", EntryRole::ContextCertificate, digest("b")),
        ])
        .expect_err("duplicate names are an error");
        assert_eq!(
            error,
            BundleError::DuplicateEntry {
                entry: "certificate".into()
            }
        );
    }

    #[test]
    fn manifest_bytes_do_not_depend_on_the_order_entries_were_supplied_in() {
        let a = ManifestEntry::new("aaa", EntryRole::World, digest("a"));
        let b = ManifestEntry::new("bbb", EntryRole::Query, digest("b"));
        let forward = manifest(vec![a.clone(), b.clone()]).expect("builds");
        let backward = manifest(vec![b, a]).expect("builds");
        assert_eq!(
            forward.digest().expect("hashes"),
            backward.digest().expect("hashes")
        );
    }

    #[test]
    fn changing_a_single_entry_digest_changes_the_manifest_digest() {
        let before = manifest(vec![ManifestEntry::new(
            "section",
            EntryRole::DecisionSection,
            digest("a"),
        )])
        .expect("builds");
        let after = manifest(vec![ManifestEntry::new(
            "section",
            EntryRole::DecisionSection,
            digest("b"),
        )])
        .expect("builds");
        assert_ne!(
            before.digest().expect("hashes"),
            after.digest().expect("hashes")
        );
    }

    #[test]
    fn a_role_that_appears_twice_is_a_cardinality_error_naming_the_role() {
        let built = manifest(vec![
            ManifestEntry::new("c1", EntryRole::ContextCertificate, digest("a")),
            ManifestEntry::new("c2", EntryRole::ContextCertificate, digest("b")),
        ])
        .expect("builds");
        assert_eq!(
            built.sole_entry_with_role(&EntryRole::ContextCertificate),
            Err(BundleError::RoleCardinality {
                role: "context_certificate".into(),
                found: 2
            })
        );
        assert_eq!(
            built.sole_entry_with_role(&EntryRole::World),
            Err(BundleError::RoleCardinality {
                role: "world".into(),
                found: 0
            })
        );
    }

    #[test]
    fn the_manifest_records_the_scheme_and_its_repudiability_in_the_hashed_bytes() {
        let built = manifest(vec![]).expect("builds");
        let bytes = String::from_utf8(built.canonical_bytes().expect("canonical")).expect("utf8");
        assert!(bytes.contains("symmetric_shared_secret"), "{bytes}");
        assert!(bytes.contains("forgeable_by_any_verifier"), "{bytes}");
        assert!(bytes.contains(BUNDLE_SCHEMA_VERSION), "{bytes}");
    }

    #[test]
    fn the_posture_separates_unrecorded_entries_from_rejected_ones() {
        use crate::provenance::{RecordedProvenance, RejectedProvenance, RejectionReason};
        let built = manifest(vec![
            ManifestEntry::new("world", EntryRole::World, digest("w")).with_provenance(
                ProvenanceState::Recorded(RecordedProvenance::new("registry://w", digest("w"))),
            ),
            ManifestEntry::new("oracle", EntryRole::Oracle, digest("o")),
            ManifestEntry::new("pack", EntryRole::BenchmarkPack, digest("p")).with_provenance(
                ProvenanceState::Rejected(RejectedProvenance::new(
                    "registry://p",
                    RejectionReason::FloatingRevision {
                        reference: "latest".into(),
                    },
                )),
            ),
        ])
        .expect("builds");
        let posture = built.supply_chain_posture();
        assert_eq!(posture.recorded, vec!["world".to_string()]);
        assert_eq!(posture.unrecorded, vec!["oracle".to_string()]);
        assert_eq!(posture.rejected, vec!["pack".to_string()]);
    }

    #[test]
    fn a_referenced_entry_records_a_locator_without_this_crate_dereferencing_it() {
        let entry = ManifestEntry::new("world", EntryRole::World, digest("w"))
            .referenced(Some("s3://worlds/w".into()));
        assert!(!entry.body.is_inline());
        assert_eq!(
            entry.body,
            EntryBody::Reference {
                locator: Some("s3://worlds/w".into())
            }
        );
    }
}
