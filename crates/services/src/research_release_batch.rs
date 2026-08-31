//! High-throughput signed research-release batch compiler.
//!
//! Atlas feature: `AFA-services-P16-F03`.
//!
//! The batch surface composes the existing single-release compiler without changing its signing
//! boundary. Requests are canonically ordered, duplicate identities are rejected before any
//! signing, and an individual release failure becomes an explicit blocked entry rather than a
//! silent omission or a false all-or-nothing success.

use crate::federation::FederationSigner;
use crate::research_release::{
    build_research_release, ResearchReleaseError, ResearchReleaseRequest,
};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-services-P16-F03";
pub const FEATURE_VERSION: &str = "0.1.0";
pub const MAX_BATCH_RELEASES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReleaseBatchRequest {
    pub releases: Vec<ResearchReleaseRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchReleaseBatchDisposition {
    Published,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReleaseBatchEntry {
    pub release_id: String,
    pub disposition: ResearchReleaseBatchDisposition,
    pub release_digest: Option<ContentHash>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReleaseBatchReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub total_releases: usize,
    pub published_releases: usize,
    pub blocked_releases: usize,
    pub entries: Vec<ResearchReleaseBatchEntry>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ResearchReleaseBatchReceipt {
    pub fn validate(&self) -> Result<(), ResearchReleaseBatchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ResearchReleaseBatchError::InvalidField(
                "schema, feature, or boundary".into(),
            ));
        }
        let classified_releases = self.published_releases.checked_add(self.blocked_releases);
        if self.total_releases == 0
            || self.total_releases != self.entries.len()
            || classified_releases != Some(self.total_releases)
            || self
                .entries
                .windows(2)
                .any(|pair| pair[0].release_id >= pair[1].release_id)
            || self.entries.iter().any(|entry| {
                entry.release_id.trim().is_empty()
                    || entry.reasons.is_empty()
                    || entry.reasons.iter().any(|reason| reason.trim().is_empty())
                    || match entry.disposition {
                        ResearchReleaseBatchDisposition::Published => {
                            entry.release_digest.is_none()
                        }
                        ResearchReleaseBatchDisposition::Blocked => entry.release_digest.is_some(),
                    }
            })
        {
            return Err(ResearchReleaseBatchError::InvalidField(
                "release counts, identity, digest, or reasons".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ResearchReleaseBatchError::Artifact(error.to_string()))?;
        if self.artifact.artifact_id != "research-release-batch"
            || self.artifact.content_type != "application/vnd.aurora.research-release-batch+json"
        {
            return Err(ResearchReleaseBatchError::Artifact(
                "artifact identity or content type does not match the release batch".into(),
            ));
        }
        let payload = json!({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "total_releases": self.total_releases,
            "published_releases": self.published_releases,
            "blocked_releases": self.blocked_releases,
            "entries": self.entries,
            "boundary": self.boundary,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| ResearchReleaseBatchError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ResearchReleaseBatchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ResearchReleaseBatchError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResearchReleaseBatchError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ResearchReleaseBatchError {
    #[error("invalid research release batch field: {0}")]
    InvalidField(String),
    #[error("duplicate release id {0}")]
    DuplicateRelease(String),
    #[error("research release batch is too large: {0} > {MAX_BATCH_RELEASES}")]
    TooLarge(usize),
    #[error("research release artifact error: {0}")]
    Artifact(String),
    #[error("research release batch serialization error: {0}")]
    Serialization(String),
}

pub fn research_release_batch_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "services".into(),
        consumers: ["publication operator".into(), "federation scheduler".into()].into(),
        behavior: "canonically batches signed research-release requests while retaining published and blocked entries with per-release reasons".into(),
        value: "makes prospective high-throughput research-object publication auditable without hiding partial federation failures".into(),
        inputs: vec![TypedPort { name: "research_release_batch_request".into(), schema: "ResearchReleaseBatchRequest@1".into(), required: true }],
        outputs: vec![TypedPort { name: "research_release_batch_receipt".into(), schema: "ResearchReleaseBatchReceipt@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: ["read:local-research-artifacts".into(), "export:policy-approved-research-object".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "consortium release approver".into(), reason: "each release retains its own explicit approval reference".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn build_research_release_batch(
    request: &ResearchReleaseBatchRequest,
    signer: &FederationSigner,
) -> Result<ResearchReleaseBatchReceipt, ResearchReleaseBatchError> {
    validate_request(request)?;
    let mut releases = request.releases.clone();
    releases.sort_by(|left, right| left.release_id.cmp(&right.release_id));
    let mut entries = Vec::with_capacity(releases.len());
    for release in releases {
        let release_id = release.release_id.clone();
        match build_research_release(&release, signer) {
            Ok(receipt) => entries.push(ResearchReleaseBatchEntry {
                release_id,
                disposition: ResearchReleaseBatchDisposition::Published,
                release_digest: Some(receipt.release_digest),
                reasons: receipt.reasons,
            }),
            Err(error) => entries.push(ResearchReleaseBatchEntry {
                release_id,
                disposition: ResearchReleaseBatchDisposition::Blocked,
                release_digest: None,
                reasons: vec![format_release_error(error)],
            }),
        }
    }
    let published_releases = entries
        .iter()
        .filter(|entry| entry.disposition == ResearchReleaseBatchDisposition::Published)
        .count();
    let blocked_releases = entries.len() - published_releases;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "total_releases": entries.len(),
        "published_releases": published_releases,
        "blocked_releases": blocked_releases,
        "entries": entries,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        "research-release-batch",
        "application/vnd.aurora.research-release-batch+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ResearchReleaseBatchError::Artifact(error.to_string()))?;
    let receipt = ResearchReleaseBatchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        total_releases: entries.len(),
        published_releases,
        blocked_releases,
        entries: serde_json::from_value(payload["entries"].clone())
            .map_err(|error| ResearchReleaseBatchError::Serialization(error.to_string()))?,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ResearchReleaseBatchRequest,
) -> Result<(), ResearchReleaseBatchError> {
    if request.releases.is_empty() {
        return Err(ResearchReleaseBatchError::InvalidField(
            "at least one release is required".into(),
        ));
    }
    if request.releases.len() > MAX_BATCH_RELEASES {
        return Err(ResearchReleaseBatchError::TooLarge(request.releases.len()));
    }
    let mut ids = BTreeSet::new();
    for release in &request.releases {
        if release.release_id.trim().is_empty() || !ids.insert(release.release_id.clone()) {
            return Err(ResearchReleaseBatchError::DuplicateRelease(
                release.release_id.clone(),
            ));
        }
    }
    Ok(())
}

fn format_release_error(error: ResearchReleaseError) -> String {
    format!("release blocked: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_release::ResearchReleaseRequest;
    use bioprism_foundation::{PolicyDecision, PolicyReceipt};

    fn policy(id: &str) -> PolicyReceipt {
        PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: format!("policy:{id}"),
            decision: PolicyDecision::Allow,
            reasons: vec!["fixture approval".into()],
            evaluated_artifacts: vec![],
            authority_reference: Some("approval:fixture".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn empty_batch_is_rejected_before_signing() {
        let signer = FederationSigner::new(
            "fixture-key",
            ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]),
        )
        .unwrap();
        assert!(build_research_release_batch(
            &ResearchReleaseBatchRequest { releases: vec![] },
            &signer
        )
        .is_err());
    }

    #[test]
    fn duplicate_release_ids_are_rejected() {
        let request = ResearchReleaseBatchRequest {
            releases: vec![
                ResearchReleaseRequest {
                    release_id: "release:a".into(),
                    origin: "site-a".into(),
                    purpose: "benchmark".into(),
                    artifacts: vec![],
                    evidence_receipts: vec![],
                    policy: policy("a"),
                    localization_statement: "local".into(),
                },
                ResearchReleaseRequest {
                    release_id: "release:a".into(),
                    origin: "site-a".into(),
                    purpose: "benchmark".into(),
                    artifacts: vec![],
                    evidence_receipts: vec![],
                    policy: policy("a-duplicate"),
                    localization_statement: "local".into(),
                },
            ],
        };
        assert!(matches!(
            validate_request(&request),
            Err(ResearchReleaseBatchError::DuplicateRelease(_))
        ));
    }

    #[test]
    fn receipt_rejects_overflow_duplicate_and_artifact_tampering() {
        let signer = FederationSigner::new(
            "fixture-key",
            ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]),
        )
        .unwrap();
        let request = ResearchReleaseBatchRequest {
            releases: vec![ResearchReleaseRequest {
                release_id: "release:a".into(),
                origin: "site-a".into(),
                purpose: "benchmark".into(),
                artifacts: vec![],
                evidence_receipts: vec![],
                policy: policy("a"),
                localization_statement: "local".into(),
            }],
        };
        let mut receipt = build_research_release_batch(&request, &signer).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(matches!(
            receipt.validate(),
            Err(ResearchReleaseBatchError::Artifact(_))
        ));

        receipt.artifact.content_hash = ContentHash::of_bytes(b"restored");
        receipt.published_releases = usize::MAX;
        receipt.blocked_releases = usize::MAX;
        assert!(matches!(
            receipt.validate(),
            Err(ResearchReleaseBatchError::InvalidField(_))
        ));
    }
}
