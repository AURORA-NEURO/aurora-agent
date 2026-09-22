//! Versioned migration checks for multimodal preclinical glioma research objects.
//!
//! A research object must remain replayable across schema editions without silently changing the
//! meaning of a modality.  This feature plans lossless metadata rewrites, explicit artifact
//! recomputation, or a fail-closed hold for every entry in a multimodal bundle.  It does not
//! rewrite artifacts, discard provenance, sign releases, or upload data.

use super::multimodal_bundle::MultimodalResearchObjectBundle;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectMigration1@1";
pub const MAX_ARTIFACTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchObjectMigrationAction {
    Preserve,
    RewriteMetadata,
    RecomputeArtifact,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchObjectMigrationDisposition {
    Compatible,
    MigrationRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObjectMigrationRequest {
    pub objective: String,
    pub bundle: MultimodalResearchObjectBundle,
    pub target_object_schema: String,
    pub target_schema_version: String,
    pub accepted_source_schema_versions: BTreeSet<String>,
    pub max_semantic_loss_milli: u16,
    pub allow_recompute: bool,
    pub require_all_modalities: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObjectMigrationDecision {
    pub artifact_id: String,
    pub modality: crate::glioma_engine::GliomaModality,
    pub source_schema_version: String,
    pub target_schema_version: String,
    pub action: ResearchObjectMigrationAction,
    pub semantic_loss_milli: u16,
    pub preserves_content_hash: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObjectMigrationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub bundle_digest: ContentHash,
    pub source_object_schema: String,
    pub target_object_schema: String,
    pub source_schema_versions: Vec<String>,
    pub target_schema_version: String,
    pub decisions: Vec<ResearchObjectMigrationDecision>,
    pub preserve_order: Vec<String>,
    pub metadata_rewrite_order: Vec<String>,
    pub recompute_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub limitations: Vec<String>,
    pub disposition: ResearchObjectMigrationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResearchObjectMigrationError {
    #[error("research-object migration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("research-object migration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("research-object migration digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_version(value: &str) -> bool {
    let mut parts = value.split('.');
    parts
        .next()
        .is_some_and(|major| !major.is_empty() && major.bytes().all(|b| b.is_ascii_digit()))
        && parts.all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn major(version: &str) -> Option<u64> {
    version.split('.').next()?.parse().ok()
}

fn digest_input(plan: &ResearchObjectMigrationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "bundle_digest": plan.bundle_digest,
        "source_object_schema": plan.source_object_schema,
        "target_object_schema": plan.target_object_schema,
        "source_schema_versions": plan.source_schema_versions,
        "target_schema_version": plan.target_schema_version,
        "decisions": plan.decisions,
        "preserve_order": plan.preserve_order,
        "metadata_rewrite_order": plan.metadata_rewrite_order,
        "recompute_order": plan.recompute_order,
        "blocked_order": plan.blocked_order,
        "omission_order": plan.omission_order,
        "limitations": plan.limitations,
        "disposition": plan.disposition,
    })
}

impl ResearchObjectMigrationPlan {
    pub fn validate(&self) -> Result<(), ResearchObjectMigrationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.bundle_digest.as_str().len() != 64
            || !valid_version(&self.target_schema_version)
            || !canonical(&self.source_schema_versions)
            || !canonical(&self.preserve_order)
            || !canonical(&self.metadata_rewrite_order)
            || !canonical(&self.recompute_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.limitations)
            || self.decisions.iter().any(|decision| {
                decision.artifact_id.trim().is_empty()
                    || !valid_version(&decision.source_schema_version)
                    || !valid_version(&decision.target_schema_version)
                    || decision.semantic_loss_milli > 1_000
                    || decision.rationale.trim().is_empty()
            })
        {
            return Err(ResearchObjectMigrationError::InvalidOutput(
                "identity, version, ordering, loss, and decision invariants are invalid".into(),
            ));
        }
        let ids = self
            .decisions
            .iter()
            .map(|decision| decision.artifact_id.clone())
            .collect::<BTreeSet<_>>();
        if ids.len() != self.decisions.len()
            || self
                .preserve_order
                .iter()
                .chain(self.metadata_rewrite_order.iter())
                .chain(self.recompute_order.iter())
                .chain(self.blocked_order.iter())
                .any(|id| !ids.contains(id))
        {
            return Err(ResearchObjectMigrationError::InvalidOutput(
                "migration partitions contain unknown or duplicate artifacts".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ResearchObjectMigrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ResearchObjectMigrationError::Digest(
                "research-object migration digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ResearchObjectMigrationRequest,
) -> Result<(), ResearchObjectMigrationError> {
    if request.objective.trim().is_empty()
        || request.target_object_schema.trim().is_empty()
        || !valid_version(&request.target_schema_version)
        || request.accepted_source_schema_versions.is_empty()
        || request
            .accepted_source_schema_versions
            .iter()
            .any(|version| !valid_version(version))
        || request.max_semantic_loss_milli > 1_000
        || request.bundle.entries.len() > MAX_ARTIFACTS
    {
        return Err(ResearchObjectMigrationError::InvalidRequest(
            "objective, target schema/version, accepted source versions, loss bound, and artifact bound are required".into(),
        ));
    }
    request
        .bundle
        .validate()
        .map_err(|error| ResearchObjectMigrationError::InvalidRequest(error.to_string()))?;
    Ok(())
}

/// Plan a fail-closed migration of every entry in a multimodal research object.
pub fn plan_glioma_research_object_migration(
    request: &ResearchObjectMigrationRequest,
) -> Result<ResearchObjectMigrationPlan, ResearchObjectMigrationError> {
    validate_request(request)?;
    let source_schema = request.bundle.output_schema.clone();
    let mut decisions = Vec::new();
    let mut preserve = BTreeSet::new();
    let mut rewrite = BTreeSet::new();
    let mut recompute = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut limitations = request
        .bundle
        .limitations
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if request.require_all_modalities && !request.bundle.missing_required_modalities.is_empty() {
        blocked.insert("required-modality-missing-before-migration".into());
    }
    if request.bundle.disposition
        == super::multimodal_bundle::MultimodalResearchObjectDisposition::Blocked
    {
        blocked.insert("source-bundle-blocked".into());
    }
    let mut source_versions = BTreeSet::new();
    for entry in &request.bundle.entries {
        source_versions.insert(entry.schema_version.clone());
        let mut action = ResearchObjectMigrationAction::Block;
        let mut loss = entry.semantic_loss_milli;
        let mut preserves_hash = false;
        let rationale;
        if !request
            .accepted_source_schema_versions
            .contains(&entry.schema_version)
        {
            rationale = "source schema version is not accepted by the migration policy".into();
            blocked.insert(format!("{}:source-schema-not-accepted", entry.artifact_id));
        } else if entry.schema_version == request.target_schema_version {
            action = ResearchObjectMigrationAction::Preserve;
            loss = entry.semantic_loss_milli;
            preserves_hash = true;
            rationale =
                "source and target schema versions match; content hash remains valid".into();
            preserve.insert(entry.artifact_id.clone());
        } else if major(&entry.schema_version) == major(&request.target_schema_version) {
            action = ResearchObjectMigrationAction::RewriteMetadata;
            loss = entry.semantic_loss_milli;
            preserves_hash = true;
            rationale = "same major schema family permits a lossless metadata rewrite".into();
            rewrite.insert(entry.artifact_id.clone());
        } else if request.allow_recompute {
            loss = entry.semantic_loss_milli.saturating_add(100);
            if loss <= request.max_semantic_loss_milli {
                action = ResearchObjectMigrationAction::RecomputeArtifact;
                rationale =
                    "major schema change requires recomputation from declared local inputs".into();
                recompute.insert(entry.artifact_id.clone());
                limitations.insert(format!("{}:content-recompute-required", entry.artifact_id));
            } else {
                rationale = "recomputation would exceed the declared semantic-loss budget".into();
                blocked.insert(format!("{}:semantic-loss-budget", entry.artifact_id));
            }
        } else {
            rationale =
                "major schema change requires recomputation, but policy disallows it".into();
            blocked.insert(format!("{}:recompute-disallowed", entry.artifact_id));
        }
        if loss > request.max_semantic_loss_milli {
            blocked.insert(format!("{}:semantic-loss:{}", entry.artifact_id, loss));
        }
        if entry.required && matches!(action, ResearchObjectMigrationAction::Block) {
            omissions.insert(format!("required-artifact-blocked:{}", entry.artifact_id));
        }
        decisions.push(ResearchObjectMigrationDecision {
            artifact_id: entry.artifact_id.clone(),
            modality: entry.modality,
            source_schema_version: entry.schema_version.clone(),
            target_schema_version: request.target_schema_version.clone(),
            action,
            semantic_loss_milli: loss,
            preserves_content_hash: preserves_hash,
            rationale,
        });
    }
    decisions.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    let disposition = if !blocked.is_empty() {
        ResearchObjectMigrationDisposition::Blocked
    } else if !recompute.is_empty() || !rewrite.is_empty() || !omissions.is_empty() {
        ResearchObjectMigrationDisposition::MigrationRequired
    } else {
        ResearchObjectMigrationDisposition::Compatible
    };
    let mut plan = ResearchObjectMigrationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        bundle_digest: request.bundle.digest.clone(),
        source_object_schema: source_schema,
        target_object_schema: request.target_object_schema.clone(),
        source_schema_versions: source_versions.into_iter().collect(),
        target_schema_version: request.target_schema_version.clone(),
        decisions,
        preserve_order: preserve.into_iter().collect(),
        metadata_rewrite_order: rewrite.into_iter().collect(),
        recompute_order: recompute.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        limitations: limitations.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| ResearchObjectMigrationError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::multimodal_bundle::{
        compile_glioma_multimodal_research_object, MultimodalResearchObjectInput,
        MultimodalResearchObjectRequest,
    };
    use crate::glioma::release::ResearchObjectRequest;
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: "application/vnd.aurora.glioma.migration+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn bundle() -> MultimodalResearchObjectBundle {
        let release = ResearchObjectRequest {
            research_id: "research-1".into(),
            study_id: "study-1".into(),
            objective: "migration".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["P03".into(), "P09".into(), "P11".into()],
            artifacts: vec![artifact("placeholder")],
            negative_evidence: Vec::new(),
            limitations: vec!["preclinical-only".into()],
            raw_data_local: true,
            aggregate_only: true,
        };
        let input = |modality, id| MultimodalResearchObjectInput {
            modality,
            artifact: artifact(id),
            source_program: "P03".into(),
            schema_version: "1.0".into(),
            semantic_loss_milli: 0,
            provenance_digest: hash("shared"),
            upstream_artifact_ids: Vec::new(),
            required: true,
        };
        compile_glioma_multimodal_research_object(&MultimodalResearchObjectRequest {
            release,
            inputs: vec![
                input(GliomaModality::Imaging, "imaging"),
                input(GliomaModality::Genomics, "genomics"),
            ],
            required_modalities: BTreeSet::from([
                GliomaModality::Imaging,
                GliomaModality::Genomics,
            ]),
            max_semantic_loss_milli: 200,
            require_cross_modal_alignment: true,
            max_inputs: 8,
        })
        .unwrap()
    }

    #[test]
    fn same_major_version_is_a_lossless_metadata_migration() {
        let plan = plan_glioma_research_object_migration(&ResearchObjectMigrationRequest {
            objective: "migration".into(),
            bundle: bundle(),
            target_object_schema: "GliomaMultimodalResearchObject1@2".into(),
            target_schema_version: "1.1".into(),
            accepted_source_schema_versions: BTreeSet::from(["1.0".into()]),
            max_semantic_loss_milli: 100,
            allow_recompute: false,
            require_all_modalities: true,
        })
        .unwrap();
        assert_eq!(
            plan.disposition,
            ResearchObjectMigrationDisposition::MigrationRequired
        );
        assert_eq!(plan.metadata_rewrite_order.len(), 2);
        assert!(plan
            .decisions
            .iter()
            .all(|decision| decision.preserves_content_hash));
        plan.validate().unwrap();
    }

    #[test]
    fn major_version_change_blocks_without_recompute_permission() {
        let plan = plan_glioma_research_object_migration(&ResearchObjectMigrationRequest {
            objective: "migration".into(),
            bundle: bundle(),
            target_object_schema: "GliomaMultimodalResearchObject1@2".into(),
            target_schema_version: "2.0".into(),
            accepted_source_schema_versions: BTreeSet::from(["1.0".into()]),
            max_semantic_loss_milli: 100,
            allow_recompute: false,
            require_all_modalities: true,
        })
        .unwrap();
        assert_eq!(
            plan.disposition,
            ResearchObjectMigrationDisposition::Blocked
        );
        assert!(!plan.blocked_order.is_empty());
    }
}
