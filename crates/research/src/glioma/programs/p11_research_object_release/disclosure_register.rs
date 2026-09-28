//! Typed, digest-bound disclosure inventory for one local research-object manifest.
//!
//! This register proves that each negative-result and limitation statement present in the
//! manifest has a stable commitment. It does not verify the scientific truth of those statements,
//! provide confidentiality (the digests are unkeyed), assess release readiness, authenticate a
//! signer, or authorize publication.

use crate::glioma::release::ResearchObjectManifest;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseDisclosureRegister1@1";
const MAX_ENTRIES: usize = 256;
const MAX_STATEMENT_CHARS: usize = 4_096;
const MAX_TOTAL_CHARS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseDisclosureKind {
    NegativeEvidence,
    Limitation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReleaseDisclosureEntry {
    pub kind: ReleaseDisclosureKind,
    pub statement_digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosureCounts {
    pub negative_evidence: usize,
    pub limitations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosureRegister {
    pub feature_id: String,
    pub output_schema: String,
    pub boundary: String,
    pub research_id: String,
    pub study_id: String,
    pub manifest_digest: ContentHash,
    pub entries: Vec<ReleaseDisclosureEntry>,
    pub counts: ReleaseDisclosureCounts,
    pub statement_text_included: bool,
    pub statement_digests_are_unkeyed: bool,
    pub scientific_truth_verified: bool,
    pub release_authorized: bool,
    pub register_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseDisclosureRegisterError {
    #[error("research-object manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("release disclosure register is invalid: {0}")]
    InvalidOutput(String),
    #[error("release disclosure register digest failed: {0}")]
    Digest(String),
}

fn statement_digest(
    kind: ReleaseDisclosureKind,
    statement: &str,
) -> Result<ContentHash, ReleaseDisclosureRegisterError> {
    ContentHash::of_value(&serde_json::json!({
        "kind": kind,
        "statement": statement,
    }))
    .map_err(|error| ReleaseDisclosureRegisterError::Digest(error.to_string()))
}

fn expected_entries(
    manifest: &ResearchObjectManifest,
) -> Result<Vec<ReleaseDisclosureEntry>, ReleaseDisclosureRegisterError> {
    let total = manifest.negative_evidence.len() + manifest.limitations.len();
    let chars = manifest
        .negative_evidence
        .iter()
        .chain(&manifest.limitations)
        .try_fold(0_usize, |sum, statement| {
            let length = statement.chars().count();
            if statement.trim().is_empty() || length > MAX_STATEMENT_CHARS {
                return None;
            }
            sum.checked_add(length)
        });
    if total == 0 || total > MAX_ENTRIES || chars.is_none_or(|value| value > MAX_TOTAL_CHARS) {
        return Err(ReleaseDisclosureRegisterError::InvalidManifest(
            "disclosures must contain 1 to 256 non-empty statements, each at most 4096 characters, with at most 65536 total characters".into(),
        ));
    }
    if manifest
        .negative_evidence
        .windows(2)
        .any(|pair| pair[0] == pair[1])
        || manifest
            .limitations
            .windows(2)
            .any(|pair| pair[0] == pair[1])
    {
        return Err(ReleaseDisclosureRegisterError::InvalidManifest(
            "disclosure statements must be unique within each category".into(),
        ));
    }

    let mut entries = manifest
        .negative_evidence
        .iter()
        .map(|statement| {
            Ok(ReleaseDisclosureEntry {
                kind: ReleaseDisclosureKind::NegativeEvidence,
                statement_digest: statement_digest(
                    ReleaseDisclosureKind::NegativeEvidence,
                    statement,
                )?,
            })
        })
        .chain(manifest.limitations.iter().map(|statement| {
            Ok(ReleaseDisclosureEntry {
                kind: ReleaseDisclosureKind::Limitation,
                statement_digest: statement_digest(ReleaseDisclosureKind::Limitation, statement)?,
            })
        }))
        .collect::<Result<Vec<_>, ReleaseDisclosureRegisterError>>()?;
    entries.sort();
    Ok(entries)
}

fn counts(entries: &[ReleaseDisclosureEntry]) -> ReleaseDisclosureCounts {
    ReleaseDisclosureCounts {
        negative_evidence: entries
            .iter()
            .filter(|entry| entry.kind == ReleaseDisclosureKind::NegativeEvidence)
            .count(),
        limitations: entries
            .iter()
            .filter(|entry| entry.kind == ReleaseDisclosureKind::Limitation)
            .count(),
    }
}

fn digest_input(register: &ReleaseDisclosureRegister) -> serde_json::Value {
    serde_json::json!({
        "feature_id": register.feature_id,
        "output_schema": register.output_schema,
        "boundary": register.boundary,
        "research_id": register.research_id,
        "study_id": register.study_id,
        "manifest_digest": register.manifest_digest,
        "entries": register.entries,
        "counts": register.counts,
        "statement_text_included": register.statement_text_included,
        "statement_digests_are_unkeyed": register.statement_digests_are_unkeyed,
        "scientific_truth_verified": register.scientific_truth_verified,
        "release_authorized": register.release_authorized,
    })
}

impl ReleaseDisclosureRegister {
    pub fn validate(
        &self,
        manifest: &ResearchObjectManifest,
    ) -> Result<(), ReleaseDisclosureRegisterError> {
        manifest
            .validate()
            .map_err(|error| ReleaseDisclosureRegisterError::InvalidManifest(error.to_string()))?;
        let expected_entries = expected_entries(manifest)?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.research_id != manifest.research_id
            || self.study_id != manifest.study_id
            || self.manifest_digest != manifest.manifest_digest
            || self.entries != expected_entries
            || self.counts != counts(&self.entries)
            || self.statement_text_included
            || !self.statement_digests_are_unkeyed
            || self.scientific_truth_verified
            || self.release_authorized
        {
            return Err(ReleaseDisclosureRegisterError::InvalidOutput(
                "identity, manifest binding, disclosure coverage, counts, or authority boundary is invalid".into(),
            ));
        }
        let expected_digest = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleaseDisclosureRegisterError::Digest(error.to_string()))?;
        if expected_digest != self.register_digest {
            return Err(ReleaseDisclosureRegisterError::InvalidOutput(
                "register digest is not bound to disclosure commitments".into(),
            ));
        }
        Ok(())
    }
}

pub fn compile_glioma_release_disclosure_register(
    manifest: &ResearchObjectManifest,
) -> Result<ReleaseDisclosureRegister, ReleaseDisclosureRegisterError> {
    manifest
        .validate()
        .map_err(|error| ReleaseDisclosureRegisterError::InvalidManifest(error.to_string()))?;
    let entries = expected_entries(manifest)?;
    let mut register = ReleaseDisclosureRegister {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        research_id: manifest.research_id.clone(),
        study_id: manifest.study_id.clone(),
        manifest_digest: manifest.manifest_digest.clone(),
        counts: counts(&entries),
        entries,
        statement_text_included: false,
        statement_digests_are_unkeyed: true,
        scientific_truth_verified: false,
        release_authorized: false,
        register_digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| ReleaseDisclosureRegisterError::Digest(error.to_string()))?,
    };
    register.register_digest = ContentHash::of_value(&digest_input(&register))
        .map_err(|error| ReleaseDisclosureRegisterError::Digest(error.to_string()))?;
    register.validate(manifest)?;
    Ok(register)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::release::{ResearchObjectRequest, build_research_object_manifest};
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({ "label": label })).unwrap()
    }

    fn manifest() -> ResearchObjectManifest {
        build_research_object_manifest(&ResearchObjectRequest {
            research_id: "research-disclosure".into(),
            study_id: "study-disclosure".into(),
            objective: "package a preclinical result".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["p10-analysis".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact-result".into(),
                content_hash: hash("result"),
                content_type: "application/vnd.aurora.glioma-result+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: vec!["effect was not reproduced".into()],
            limitations: vec![
                "one model system was evaluated".into(),
                "short follow-up".into(),
            ],
            raw_data_local: true,
            aggregate_only: true,
        })
        .unwrap()
    }

    #[test]
    fn register_is_canonical_manifest_bound_and_preserves_category_counts() {
        let manifest = manifest();
        let register = compile_glioma_release_disclosure_register(&manifest).unwrap();
        assert_eq!(register.entries.len(), 3);
        assert_eq!(register.counts.negative_evidence, 1);
        assert_eq!(register.counts.limitations, 2);
        assert_eq!(register.manifest_digest, manifest.manifest_digest);
        assert!(!register.statement_text_included);
        assert!(register.statement_digests_are_unkeyed);
        assert!(!register.scientific_truth_verified);
        assert!(!register.release_authorized);
        register.validate(&manifest).unwrap();
    }

    #[test]
    fn register_digest_and_manifest_binding_reject_tampering() {
        let manifest = manifest();
        let mut register = compile_glioma_release_disclosure_register(&manifest).unwrap();
        register.entries.pop();
        assert!(register.validate(&manifest).is_err());

        let mut digest_tampered = compile_glioma_release_disclosure_register(&manifest).unwrap();
        digest_tampered.register_digest = hash("tampered-register");
        assert!(digest_tampered.validate(&manifest).is_err());

        let register = compile_glioma_release_disclosure_register(&manifest).unwrap();
        let mut other_manifest = manifest.clone();
        other_manifest.study_id = "different-study".into();
        other_manifest.manifest_digest =
            ContentHash::of_value(&crate::glioma::release::digest_input(&other_manifest)).unwrap();
        assert!(register.validate(&other_manifest).is_err());
    }

    #[test]
    fn compiler_rejects_unbounded_or_duplicate_disclosures() {
        let mut duplicate_manifest = manifest();
        duplicate_manifest.limitations = vec!["duplicate".into(), "duplicate".into()];
        duplicate_manifest.limitations.sort();
        duplicate_manifest.manifest_digest =
            ContentHash::of_value(&crate::glioma::release::digest_input(&duplicate_manifest))
                .unwrap();
        assert!(compile_glioma_release_disclosure_register(&duplicate_manifest).is_err());

        let mut oversized = manifest();
        oversized.limitations = vec!["x".repeat(MAX_STATEMENT_CHARS + 1)];
        oversized.manifest_digest =
            ContentHash::of_value(&crate::glioma::release::digest_input(&oversized)).unwrap();
        assert!(compile_glioma_release_disclosure_register(&oversized).is_err());
    }
}
