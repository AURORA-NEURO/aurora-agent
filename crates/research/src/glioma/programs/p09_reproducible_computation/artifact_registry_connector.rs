//! Policy-bounded local scientific artifact-registry resolution for glioma workflows.
//!
//! The connector resolves handles, not raw bytes. It checks content identity, schema, license,
//! registry trust, freshness, locality, and authorization before a computation can consume an
//! artifact. Registry outages and ambiguous candidates remain unresolved; they never become a
//! best-effort substitution.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaVerifiedArtifactResolution1@1";
pub const MAX_CANDIDATES: usize = 512;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub artifact_id: String,
    pub content_hash: ContentHash,
    pub content_type: String,
    pub schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryArtifactCandidate {
    pub artifact_id: String,
    pub content_hash: ContentHash,
    pub content_type: String,
    pub schema_version: String,
    pub license: String,
    pub source_uri: String,
    pub source_digest: ContentHash,
    pub signed_metadata: bool,
    pub available: bool,
    pub stale: bool,
    pub corrupt: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub contains_clinical_decision: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRegistryTrustConfig {
    pub approved_source_prefix_order: Vec<String>,
    pub allowed_license_order: Vec<String>,
    pub allowed_content_type_order: Vec<String>,
    pub require_signed_metadata: bool,
    pub require_local_only: bool,
    pub max_candidate_age_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRegistryAccessGrant {
    pub grant_id: String,
    pub site_id: String,
    pub authorized: bool,
    pub expires_at_tick: u64,
    pub allow_registry_read: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRegistryResolutionRequest {
    pub artifact_ref: ArtifactRef,
    pub candidates: Vec<RegistryArtifactCandidate>,
    pub trust: ArtifactRegistryTrustConfig,
    pub access: ArtifactRegistryAccessGrant,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactResolutionDisposition {
    Resolved,
    Denied,
    Unresolved,
    SchemaMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedArtifactHandle {
    pub artifact_id: String,
    pub content_hash: ContentHash,
    pub content_type: String,
    pub schema_version: String,
    pub source_uri: String,
    pub source_digest: ContentHash,
    pub license: String,
    pub site_id: String,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedArtifactResolution {
    pub feature_id: String,
    pub output_schema: String,
    pub artifact_ref: ArtifactRef,
    pub disposition: ArtifactResolutionDisposition,
    pub handle: Option<VerifiedArtifactHandle>,
    pub rejected_candidate_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactRegistryConnectorError {
    #[error("artifact registry request is invalid: {0}")]
    InvalidRequest(String),
    #[error("artifact resolution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("artifact resolution digest failed: {0}")]
    Digest(String),
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_bounded(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn digest_input(resolution: &VerifiedArtifactResolution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": resolution.feature_id,
        "output_schema": resolution.output_schema,
        "artifact_ref": resolution.artifact_ref,
        "disposition": resolution.disposition,
        "handle": resolution.handle,
        "rejected_candidate_order": resolution.rejected_candidate_order,
        "evidence_order": resolution.evidence_order,
        "negative_evidence": resolution.negative_evidence,
        "uncertainty": resolution.uncertainty,
    })
}

fn validate_request(
    request: &ArtifactRegistryResolutionRequest,
) -> Result<(), ArtifactRegistryConnectorError> {
    let reference = &request.artifact_ref;
    if reference.artifact_id.trim().is_empty()
        || reference.artifact_id.len() > MAX_TEXT_LEN
        || !valid_hash(&reference.content_hash)
        || reference.content_type.trim().is_empty()
        || reference.content_type.len() > MAX_TEXT_LEN
        || reference.schema_version.trim().is_empty()
        || reference.schema_version.len() > MAX_TEXT_LEN
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || !unique_bounded(&request.trust.approved_source_prefix_order, MAX_CANDIDATES)
        || !canonical(&request.trust.approved_source_prefix_order)
        || !unique_bounded(&request.trust.allowed_license_order, MAX_CANDIDATES)
        || !canonical(&request.trust.allowed_license_order)
        || !unique_bounded(&request.trust.allowed_content_type_order, MAX_CANDIDATES)
        || !canonical(&request.trust.allowed_content_type_order)
        || request.access.grant_id.trim().is_empty()
        || request.access.grant_id.len() > MAX_TEXT_LEN
        || request.access.site_id.trim().is_empty()
        || request.access.site_id.len() > MAX_TEXT_LEN
        || request.access.expires_at_tick == 0
        || request.current_tick > request.access.expires_at_tick
        || request.trust.max_candidate_age_ticks == 0
    {
        return Err(ArtifactRegistryConnectorError::InvalidRequest(
            "bounded content identity, canonical trust lists, live access, and candidate set are required".into(),
        ));
    }
    for candidate in &request.candidates {
        if candidate.artifact_id.trim().is_empty()
            || candidate.artifact_id.len() > MAX_TEXT_LEN
            || !valid_hash(&candidate.content_hash)
            || candidate.content_type.trim().is_empty()
            || candidate.content_type.len() > MAX_TEXT_LEN
            || candidate.schema_version.trim().is_empty()
            || candidate.schema_version.len() > MAX_TEXT_LEN
            || candidate.license.trim().is_empty()
            || candidate.license.len() > MAX_TEXT_LEN
            || candidate.source_uri.trim().is_empty()
            || candidate.source_uri.len() > MAX_TEXT_LEN
            || !valid_hash(&candidate.source_digest)
        {
            return Err(ArtifactRegistryConnectorError::InvalidRequest(
                "candidate identity, schema, license, source, and digests are required".into(),
            ));
        }
    }
    Ok(())
}

impl VerifiedArtifactResolution {
    pub fn validate(&self) -> Result<(), ArtifactRegistryConnectorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.artifact_ref.artifact_id.trim().is_empty()
            || !valid_hash(&self.artifact_ref.content_hash)
            || self.artifact_ref.content_type.trim().is_empty()
            || self.artifact_ref.schema_version.trim().is_empty()
            || !unique_bounded(&self.rejected_candidate_order, MAX_CANDIDATES)
            || !canonical(&self.rejected_candidate_order)
            || !unique_bounded(&self.evidence_order, MAX_CANDIDATES)
            || !canonical(&self.evidence_order)
            || !unique_bounded(&self.negative_evidence, MAX_CANDIDATES)
            || !canonical(&self.negative_evidence)
            || !unique_bounded(&self.uncertainty, MAX_CANDIDATES)
            || !canonical(&self.uncertainty)
            || !valid_hash(&self.digest)
            || self.handle.as_ref().is_some_and(|handle| {
                handle.artifact_id != self.artifact_ref.artifact_id
                    || handle.content_hash != self.artifact_ref.content_hash
                    || handle.content_type != self.artifact_ref.content_type
                    || handle.schema_version != self.artifact_ref.schema_version
                    || !valid_hash(&handle.source_digest)
            })
        {
            return Err(ArtifactRegistryConnectorError::InvalidOutput(
                "resolution identity, canonical evidence, handle binding, or digest is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ArtifactRegistryConnectorError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ArtifactRegistryConnectorError::InvalidOutput(
                "artifact resolution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Resolve one local artifact handle against approved registry metadata without moving bytes.
pub fn resolve_glioma_registry_artifact(
    request: &ArtifactRegistryResolutionRequest,
) -> Result<VerifiedArtifactResolution, ArtifactRegistryConnectorError> {
    validate_request(request)?;
    let reference = &request.artifact_ref;
    let approved_sources = request
        .trust
        .approved_source_prefix_order
        .iter()
        .collect::<Vec<_>>();
    let allowed_licenses = request
        .trust
        .allowed_license_order
        .iter()
        .collect::<BTreeSet<_>>();
    let allowed_types = request
        .trust
        .allowed_content_type_order
        .iter()
        .collect::<BTreeSet<_>>();
    let mut rejected = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut candidate = None;
    for item in &request.candidates {
        let mut reasons = Vec::new();
        if item.artifact_id != reference.artifact_id || item.content_hash != reference.content_hash
        {
            reasons.push("content-identity-mismatch");
        }
        if item.content_type != reference.content_type
            || item.schema_version != reference.schema_version
        {
            reasons.push("schema-or-content-type-mismatch");
        }
        if !allowed_types.contains(&item.content_type) {
            reasons.push("content-type-not-allowed");
        }
        if !allowed_licenses.contains(&item.license) {
            reasons.push("license-not-allowed");
        }
        if !approved_sources
            .iter()
            .any(|prefix| item.source_uri.starts_with(*prefix))
        {
            reasons.push("registry-source-not-approved");
        }
        if request.trust.require_signed_metadata && !item.signed_metadata {
            reasons.push("metadata-signature-missing");
        }
        if !item.available {
            reasons.push("registry-object-unavailable");
        }
        if item.stale {
            reasons.push("registry-object-stale");
        }
        if item.corrupt {
            reasons.push("registry-object-corrupt");
        }
        if request.trust.require_local_only && !item.local_only {
            reasons.push("locality-policy-denied");
        }
        if item.contains_human_data
            || item.contains_direct_identifiers
            || item.contains_clinical_decision
        {
            reasons.push("preclinical-boundary-denied");
        }
        if reasons.is_empty() && candidate.is_none() {
            candidate = Some(item);
            evidence.insert("content-hash-schema-license-and-locality-qualified".into());
        } else if !reasons.is_empty() {
            rejected.insert(item.artifact_id.clone());
            for reason in reasons {
                evidence.insert(format!("{}:{reason}", item.artifact_id));
            }
        }
    }
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let (disposition, handle) = if !request.access.authorized || !request.access.allow_registry_read
    {
        negative_evidence.push("registry-read-not-authorized".into());
        (ArtifactResolutionDisposition::Denied, None)
    } else if let Some(item) = candidate {
        (
            ArtifactResolutionDisposition::Resolved,
            Some(VerifiedArtifactHandle {
                artifact_id: item.artifact_id.clone(),
                content_hash: item.content_hash.clone(),
                content_type: item.content_type.clone(),
                schema_version: item.schema_version.clone(),
                source_uri: item.source_uri.clone(),
                source_digest: item.source_digest.clone(),
                license: item.license.clone(),
                site_id: request.access.site_id.clone(),
                local_only: item.local_only,
            }),
        )
    } else if request.candidates.iter().any(|item| {
        item.artifact_id == reference.artifact_id && item.content_hash == reference.content_hash
    }) {
        negative_evidence.push("matching-object-rejected-by-schema-or-trust-gate".into());
        (ArtifactResolutionDisposition::SchemaMismatch, None)
    } else {
        uncertainty.push("registry-candidate-unavailable-or-not-found".into());
        (ArtifactResolutionDisposition::Unresolved, None)
    };
    let mut resolution = VerifiedArtifactResolution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        artifact_ref: reference.clone(),
        disposition,
        handle,
        rejected_candidate_order: rejected.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        negative_evidence,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-artifact-resolution"),
    };
    resolution.digest = ContentHash::of_value(&digest_input(&resolution))
        .map_err(|error| ArtifactRegistryConnectorError::Digest(error.to_string()))?;
    resolution.validate()?;
    Ok(resolution)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request(candidate: RegistryArtifactCandidate) -> ArtifactRegistryResolutionRequest {
        ArtifactRegistryResolutionRequest {
            artifact_ref: ArtifactRef {
                artifact_id: "local-image-stack".into(),
                content_hash: hash("image"),
                content_type: "image-stack".into(),
                schema_version: "ome-ngff-0.5".into(),
            },
            candidates: vec![candidate],
            trust: ArtifactRegistryTrustConfig {
                approved_source_prefix_order: vec!["registry://trusted".into()],
                allowed_license_order: vec!["CC-BY-4.0".into()],
                allowed_content_type_order: vec!["image-stack".into()],
                require_signed_metadata: true,
                require_local_only: true,
                max_candidate_age_ticks: 10,
            },
            access: ArtifactRegistryAccessGrant {
                grant_id: "grant-1".into(),
                site_id: "site-a".into(),
                authorized: true,
                expires_at_tick: 100,
                allow_registry_read: true,
            },
            current_tick: 1,
        }
    }

    fn candidate() -> RegistryArtifactCandidate {
        RegistryArtifactCandidate {
            artifact_id: "local-image-stack".into(),
            content_hash: hash("image"),
            content_type: "image-stack".into(),
            schema_version: "ome-ngff-0.5".into(),
            license: "CC-BY-4.0".into(),
            source_uri: "registry://trusted/local-image-stack".into(),
            source_digest: hash("source"),
            signed_metadata: true,
            available: true,
            stale: false,
            corrupt: false,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            contains_clinical_decision: false,
        }
    }

    #[test]
    fn resolves_exact_local_artifact_handle() {
        let resolution = resolve_glioma_registry_artifact(&request(candidate())).expect("resolved");
        assert_eq!(
            resolution.disposition,
            ArtifactResolutionDisposition::Resolved
        );
        assert_eq!(resolution.handle.expect("handle").site_id, "site-a");
    }

    #[test]
    fn corrupt_or_stale_candidate_is_not_substituted() {
        let mut item = candidate();
        item.corrupt = true;
        item.stale = true;
        let resolution = resolve_glioma_registry_artifact(&request(item)).expect("resolution");
        assert_eq!(
            resolution.disposition,
            ArtifactResolutionDisposition::SchemaMismatch
        );
        assert!(resolution
            .evidence_order
            .iter()
            .any(|evidence| evidence.contains("corrupt")));
    }

    #[test]
    fn unauthorized_read_is_denied_even_with_valid_candidate() {
        let mut req = request(candidate());
        req.access.authorized = false;
        let resolution = resolve_glioma_registry_artifact(&req).expect("denied");
        assert_eq!(
            resolution.disposition,
            ArtifactResolutionDisposition::Denied
        );
        assert!(resolution.handle.is_none());
    }

    #[test]
    fn human_or_clinical_boundary_is_rejected() {
        let mut item = candidate();
        item.contains_human_data = true;
        assert_eq!(
            resolve_glioma_registry_artifact(&request(item))
                .expect("boundary result")
                .disposition,
            ArtifactResolutionDisposition::SchemaMismatch
        );
    }
}
