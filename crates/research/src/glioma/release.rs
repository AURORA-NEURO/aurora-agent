//! Portable research-object release preparation.
//!
//! This program packages the outputs of the other glioma programs for a registry or consortium
//! review.  It never claims a signature it did not receive: the result is either
//! `ready_for_signing` or blocked with the exact missing release inputs.

use super::super::glioma_engine::LocalArtifactRef;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectManifest1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObjectRequest {
    pub research_id: String,
    pub study_id: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub execution_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub program_order: Vec<String>,
    pub artifacts: Vec<LocalArtifactRef>,
    pub negative_evidence: Vec<String>,
    pub limitations: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    ReadyForSigning,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObjectManifest {
    pub feature_id: String,
    pub output_schema: String,
    pub boundary: String,
    pub research_id: String,
    pub study_id: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub execution_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub program_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub limitations: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub signing_status: String,
    pub blocked_order: Vec<String>,
    pub release_status: ReleaseStatus,
    pub manifest_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseError {
    #[error("research-object request is invalid: {0}")]
    InvalidRequest(String),
    #[error("research-object manifest is invalid: {0}")]
    InvalidOutput(String),
    #[error("research-object digest failed: {0}")]
    Digest(String),
}

fn digest_input(manifest: &ResearchObjectManifest) -> serde_json::Value {
    serde_json::json!({
        "feature_id": manifest.feature_id,
        "output_schema": manifest.output_schema,
        "boundary": manifest.boundary,
        "research_id": manifest.research_id,
        "study_id": manifest.study_id,
        "objective": manifest.objective,
        "plan_digest": manifest.plan_digest,
        "execution_digest": manifest.execution_digest,
        "replay_identity": manifest.replay_identity,
        "program_order": manifest.program_order,
        "artifact_order": manifest.artifact_order,
        "negative_evidence": manifest.negative_evidence,
        "limitations": manifest.limitations,
        "raw_data_local": manifest.raw_data_local,
        "aggregate_only": manifest.aggregate_only,
        "signing_status": manifest.signing_status,
        "blocked_order": manifest.blocked_order,
        "release_status": manifest.release_status,
    })
}

impl ResearchObjectManifest {
    pub fn validate(&self) -> Result<(), ReleaseError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.research_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.program_order.is_empty()
            || self.artifact_order.is_empty()
            || self.program_order.windows(2).any(|pair| pair[0] > pair[1])
            || self.artifact_order.windows(2).any(|pair| pair[0] > pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.limitations.windows(2).any(|pair| pair[0] > pair[1])
            || self.blocked_order.windows(2).any(|pair| pair[0] > pair[1])
            || self.signing_status != "unsigned-pending-accountable-signature"
        {
            return Err(ReleaseError::InvalidOutput("identity, boundary, artifact/feature ordering, limitations, or signing status is invalid".into()));
        }
        for digest in [
            &self.plan_digest,
            &self.execution_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ReleaseError::InvalidOutput(
                    "release digests must be SHA-256 values".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| ReleaseError::Digest(e.to_string()))?;
        if expected != self.manifest_digest {
            return Err(ReleaseError::InvalidOutput(
                "manifest digest is not bound to release contents".into(),
            ));
        }
        Ok(())
    }
}

pub fn build_research_object_manifest(
    request: &ResearchObjectRequest,
) -> Result<ResearchObjectManifest, ReleaseError> {
    if request.research_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.program_order.is_empty()
        || request.artifacts.is_empty()
        || !request.raw_data_local
        || request.plan_digest.as_str().len() != 64
        || request.execution_digest.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ReleaseError::InvalidRequest(
            "research identity, digests, artifacts, program order, and locality are required"
                .into(),
        ));
    }
    let mut programs = request.program_order.clone();
    programs.sort();
    if programs.windows(2).any(|pair| pair[0] == pair[1])
        || programs.iter().any(|program| program.trim().is_empty())
    {
        return Err(ReleaseError::InvalidRequest(
            "program order must be unique and non-empty".into(),
        ));
    }
    let mut artifacts = BTreeSet::new();
    for artifact in &request.artifacts {
        artifact
            .validate()
            .map_err(|e| ReleaseError::InvalidRequest(e.to_string()))?;
        artifacts.insert(artifact.artifact_id.clone());
    }
    let mut blocked = BTreeSet::new();
    let mut limitations = request.limitations.iter().cloned().collect::<BTreeSet<_>>();
    if request.aggregate_only && !request.raw_data_local {
        blocked.insert("aggregate-export-requires-local-raw-data".into());
    }
    if request.limitations.is_empty() {
        blocked.insert("at-least-one-limitation-required".into());
        limitations.insert("limitation-missing".into());
    }
    let release_status = if blocked.is_empty() {
        ReleaseStatus::ReadyForSigning
    } else {
        ReleaseStatus::Blocked
    };
    let mut manifest = ResearchObjectManifest {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        research_id: request.research_id.clone(),
        study_id: request.study_id.clone(),
        objective: request.objective.clone(),
        plan_digest: request.plan_digest.clone(),
        execution_digest: request.execution_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        program_order: programs,
        artifact_order: artifacts.into_iter().collect(),
        negative_evidence: {
            let mut values = request.negative_evidence.clone();
            values.sort();
            values
        },
        limitations: limitations.into_iter().collect(),
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        signing_status: "unsigned-pending-accountable-signature".into(),
        blocked_order: blocked.into_iter().collect(),
        release_status,
        manifest_digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| ReleaseError::Digest(e.to_string()))?,
    };
    manifest.manifest_digest = ContentHash::of_value(&digest_input(&manifest))
        .map_err(|e| ReleaseError::Digest(e.to_string()))?;
    manifest.validate()?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> ResearchObjectRequest {
        ResearchObjectRequest {
            research_id: "research-1".into(),
            study_id: "study-1".into(),
            objective: "release a preclinical glioma result".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["p05-mechanism".into(), "p10-analysis".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact-1".into(),
                content_hash: hash("artifact"),
                content_type: "application/vnd.aurora.glioma-result+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: vec!["null-result-preserved".into()],
            limitations: vec!["single-model-system".into()],
            raw_data_local: true,
            aggregate_only: true,
        }
    }

    #[test]
    fn manifest_is_ready_for_accountable_signing_but_never_falsifies_a_signature() {
        let manifest = build_research_object_manifest(&request()).unwrap();
        assert_eq!(manifest.release_status, ReleaseStatus::ReadyForSigning);
        assert_eq!(
            manifest.signing_status,
            "unsigned-pending-accountable-signature"
        );
        manifest.validate().unwrap();
    }

    #[test]
    fn missing_limitations_block_release() {
        let mut request = request();
        request.limitations.clear();
        let manifest = build_research_object_manifest(&request).unwrap();
        assert_eq!(manifest.release_status, ReleaseStatus::Blocked);
        assert_eq!(
            manifest.blocked_order,
            vec!["at-least-one-limitation-required"]
        );
    }
}
