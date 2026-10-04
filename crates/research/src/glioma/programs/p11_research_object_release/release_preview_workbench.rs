//! Audience-specific research-object release previews.
//!
//! The preview is a product artifact for a researcher or release reviewer: it computes exactly
//! which sections and artifacts an audience would receive, which are redacted, and how that
//! selection differs from a prior preview.  It never renders protected payloads, signs, uploads,
//! or silently treats an omitted section as present.

use crate::glioma::release::ResearchObjectManifest;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseAudiencePreview1@1";
pub const MAX_SECTIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewComparison {
    NewRelease,
    ComparedToPrior,
    PriorUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasePreviewRequest {
    pub manifest: ResearchObjectManifest,
    pub audience_id: String,
    pub export_profile: String,
    pub allowed_section_order: Vec<String>,
    pub redact_section_order: Vec<String>,
    pub allowed_artifact_order: Vec<String>,
    pub prior_preview_digest: Option<ContentHash>,
    pub prior_section_order: Vec<String>,
    pub prior_artifact_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAudiencePreview {
    pub feature_id: String,
    pub output_schema: String,
    pub audience_id: String,
    pub export_profile: String,
    pub manifest_digest: ContentHash,
    pub rendered_section_order: Vec<String>,
    pub redacted_section_order: Vec<String>,
    pub rendered_artifact_order: Vec<String>,
    pub omitted_artifact_order: Vec<String>,
    pub diff_added_section_order: Vec<String>,
    pub diff_removed_section_order: Vec<String>,
    pub diff_added_artifact_order: Vec<String>,
    pub diff_removed_artifact_order: Vec<String>,
    pub comparison: PreviewComparison,
    pub redacted_payload_present: bool,
    pub preview_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleasePreviewError {
    #[error("release preview request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release preview output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release preview digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(preview: &ReleaseAudiencePreview) -> serde_json::Value {
    serde_json::json!({
        "feature_id": preview.feature_id,
        "output_schema": preview.output_schema,
        "audience_id": preview.audience_id,
        "export_profile": preview.export_profile,
        "manifest_digest": preview.manifest_digest,
        "rendered_section_order": preview.rendered_section_order,
        "redacted_section_order": preview.redacted_section_order,
        "rendered_artifact_order": preview.rendered_artifact_order,
        "omitted_artifact_order": preview.omitted_artifact_order,
        "diff_added_section_order": preview.diff_added_section_order,
        "diff_removed_section_order": preview.diff_removed_section_order,
        "diff_added_artifact_order": preview.diff_added_artifact_order,
        "diff_removed_artifact_order": preview.diff_removed_artifact_order,
        "comparison": preview.comparison,
        "redacted_payload_present": preview.redacted_payload_present,
    })
}

impl ReleaseAudiencePreview {
    pub fn validate(&self) -> Result<(), ReleasePreviewError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.audience_id)
            || !identifier(&self.export_profile)
            || self.manifest_digest.as_str().len() != 64
            || !canonical(&self.rendered_section_order)
            || !canonical(&self.redacted_section_order)
            || !canonical(&self.rendered_artifact_order)
            || !canonical(&self.omitted_artifact_order)
            || !canonical(&self.diff_added_section_order)
            || !canonical(&self.diff_removed_section_order)
            || !canonical(&self.diff_added_artifact_order)
            || !canonical(&self.diff_removed_artifact_order)
            || self.redacted_payload_present
            || self.preview_digest.as_str().len() != 64
        {
            return Err(ReleasePreviewError::InvalidOutput(
                "preview identity, canonical partitions, protected-payload boundary, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleasePreviewError::Digest(error.to_string()))?;
        if expected != self.preview_digest {
            return Err(ReleasePreviewError::InvalidOutput(
                "preview digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ReleasePreviewRequest) -> Result<(), ReleasePreviewError> {
    request
        .manifest
        .validate()
        .map_err(|error| ReleasePreviewError::InvalidRequest(error.to_string()))?;
    if !identifier(&request.audience_id)
        || !identifier(&request.export_profile)
        || request.allowed_section_order.is_empty()
        || request.allowed_section_order.len() > MAX_SECTIONS
        || !canonical(&request.allowed_section_order)
        || !canonical(&request.redact_section_order)
        || !canonical(&request.allowed_artifact_order)
        || !canonical(&request.prior_section_order)
        || !canonical(&request.prior_artifact_order)
        || request
            .prior_preview_digest
            .as_ref()
            .is_some_and(|digest| digest.as_str().len() != 64)
        || request
            .redact_section_order
            .iter()
            .any(|section| !request.allowed_section_order.contains(section))
    {
        return Err(ReleasePreviewError::InvalidRequest(
            "audience/profile, canonical bounded sections, artifact selection, and valid prior preview are required".into(),
        ));
    }
    Ok(())
}

/// Compile the exact audience-specific preview that a release profile would expose.
pub fn preview_glioma_release_audience(
    request: &ReleasePreviewRequest,
) -> Result<ReleaseAudiencePreview, ReleasePreviewError> {
    validate_request(request)?;
    let redacted = request
        .redact_section_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let rendered_section_order = request
        .allowed_section_order
        .iter()
        .filter(|section| !redacted.contains(*section))
        .cloned()
        .collect::<Vec<_>>();
    let manifest_artifacts = request
        .manifest
        .artifact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let allowed_artifacts = request
        .allowed_artifact_order
        .iter()
        .filter(|artifact| manifest_artifacts.contains(*artifact))
        .cloned()
        .collect::<Vec<_>>();
    let omitted_artifact_order = manifest_artifacts
        .difference(&allowed_artifacts.iter().cloned().collect::<BTreeSet<_>>())
        .cloned()
        .collect::<Vec<_>>();
    let prior_available = request.prior_preview_digest.is_some();
    let prior_sections = request
        .prior_section_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let current_sections = rendered_section_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let prior_artifacts = request
        .prior_artifact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let current_artifacts = allowed_artifacts.iter().cloned().collect::<BTreeSet<_>>();
    let comparison = if !prior_available {
        PreviewComparison::NewRelease
    } else {
        PreviewComparison::ComparedToPrior
    };
    let mut output = ReleaseAudiencePreview {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        audience_id: request.audience_id.clone(),
        export_profile: request.export_profile.clone(),
        manifest_digest: request.manifest.manifest_digest.clone(),
        rendered_section_order,
        redacted_section_order: redacted.into_iter().collect(),
        rendered_artifact_order: allowed_artifacts,
        omitted_artifact_order,
        diff_added_section_order: current_sections
            .difference(&prior_sections)
            .cloned()
            .collect(),
        diff_removed_section_order: prior_sections
            .difference(&current_sections)
            .cloned()
            .collect(),
        diff_added_artifact_order: current_artifacts
            .difference(&prior_artifacts)
            .cloned()
            .collect(),
        diff_removed_artifact_order: prior_artifacts
            .difference(&current_artifacts)
            .cloned()
            .collect(),
        comparison,
        redacted_payload_present: false,
        preview_digest: ContentHash::of_bytes(b"unsealed-glioma-release-audience-preview"),
    };
    output.preview_digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReleasePreviewError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::release::{build_research_object_manifest, ResearchObjectRequest};
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn manifest() -> ResearchObjectManifest {
        build_research_object_manifest(&ResearchObjectRequest {
            research_id: "preview-research".into(),
            study_id: "preview-study".into(),
            objective: "preview an aggregate glioma release".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["p05".into(), "p10".into()],
            artifacts: vec![
                LocalArtifactRef {
                    artifact_id: "artifact-a".into(),
                    content_hash: hash("a"),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
                LocalArtifactRef {
                    artifact_id: "artifact-b".into(),
                    content_hash: hash("b"),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            ],
            negative_evidence: vec!["null-result".into()],
            limitations: vec!["single-model".into()],
            raw_data_local: true,
            aggregate_only: true,
        })
        .unwrap()
    }

    fn request() -> ReleasePreviewRequest {
        ReleasePreviewRequest {
            manifest: manifest(),
            audience_id: "consortium-review".into(),
            export_profile: "aggregate-review-v1".into(),
            allowed_section_order: vec!["limitations".into(), "methods".into(), "results".into()],
            redact_section_order: vec!["results".into()],
            allowed_artifact_order: vec!["artifact-a".into()],
            prior_preview_digest: Some(hash("prior")),
            prior_section_order: vec!["methods".into()],
            prior_artifact_order: vec!["artifact-b".into()],
        }
    }

    #[test]
    fn preview_matches_audience_and_redaction_policy() {
        let output = preview_glioma_release_audience(&request()).unwrap();
        assert_eq!(
            output.rendered_section_order,
            vec!["limitations", "methods"]
        );
        assert_eq!(output.redacted_section_order, vec!["results"]);
        assert_eq!(output.rendered_artifact_order, vec!["artifact-a"]);
        assert!(!output.redacted_payload_present);
        output.validate().unwrap();
    }

    #[test]
    fn preview_reports_prior_diff_without_claiming_comparability() {
        let output = preview_glioma_release_audience(&request()).unwrap();
        assert_eq!(output.comparison, PreviewComparison::ComparedToPrior);
        assert_eq!(output.diff_added_section_order, vec!["limitations"]);
        assert_eq!(output.diff_removed_section_order, Vec::<String>::new());
        assert_eq!(output.diff_added_artifact_order, vec!["artifact-a"]);
        assert_eq!(output.diff_removed_artifact_order, vec!["artifact-b"]);
    }

    #[test]
    fn unknown_or_redacted_artifacts_are_omitted() {
        let mut request = request();
        request
            .allowed_artifact_order
            .push("not-in-manifest".into());
        request.allowed_artifact_order.sort();
        let output = preview_glioma_release_audience(&request).unwrap();
        assert_eq!(output.rendered_artifact_order, vec!["artifact-a"]);
        assert_eq!(output.omitted_artifact_order, vec!["artifact-b"]);
    }

    #[test]
    fn preview_mutation_breaks_digest() {
        let mut output = preview_glioma_release_audience(&request()).unwrap();
        output.audience_id = "mutated".into();
        assert!(matches!(
            output.validate(),
            Err(ReleasePreviewError::InvalidOutput(_))
        ));
    }
}
