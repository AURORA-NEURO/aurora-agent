//! Multimodal research-object packaging for preclinical glioma studies.
//!
//! This feature compiles modality-specific local artifacts into one release-ready object while
//! retaining semantic-loss budgets, source-program provenance, required-modality omissions, and
//! cross-modal alignment evidence.  It never uploads or signs a bundle: it produces the typed
//! object that the existing release gate and institution-owned signer can review.

use crate::glioma::release::{
    build_research_object_manifest, ReleaseStatus, ResearchObjectManifest, ResearchObjectRequest,
};
use crate::glioma_engine::{GliomaModality, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalResearchObject1@1";
pub const MAX_INPUTS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalResearchObjectInput {
    pub modality: GliomaModality,
    pub artifact: LocalArtifactRef,
    pub source_program: String,
    pub schema_version: String,
    pub semantic_loss_milli: u16,
    pub provenance_digest: ContentHash,
    pub upstream_artifact_ids: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalResearchObjectRequest {
    pub release: ResearchObjectRequest,
    pub inputs: Vec<MultimodalResearchObjectInput>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub max_semantic_loss_milli: u16,
    pub require_cross_modal_alignment: bool,
    pub max_inputs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalResearchObjectEntry {
    pub modality: GliomaModality,
    pub artifact_id: String,
    pub content_hash: ContentHash,
    pub source_program: String,
    pub schema_version: String,
    pub semantic_loss_milli: u16,
    pub provenance_digest: ContentHash,
    pub upstream_artifact_ids: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalResearchObjectDisposition {
    ReadyForSigning,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalResearchObjectBundle {
    pub feature_id: String,
    pub output_schema: String,
    pub manifest: ResearchObjectManifest,
    pub modality_order: Vec<GliomaModality>,
    pub entries: Vec<MultimodalResearchObjectEntry>,
    pub covered_required_modalities: Vec<GliomaModality>,
    pub missing_required_modalities: Vec<GliomaModality>,
    pub alignment_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub limitations: Vec<String>,
    pub disposition: MultimodalResearchObjectDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalResearchObjectError {
    #[error("multimodal research-object request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal research-object manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("multimodal research-object output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal research-object digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}

fn digest_input(bundle: &MultimodalResearchObjectBundle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": bundle.feature_id,
        "output_schema": bundle.output_schema,
        "manifest": bundle.manifest,
        "modality_order": bundle.modality_order,
        "entries": bundle.entries,
        "covered_required_modalities": bundle.covered_required_modalities,
        "missing_required_modalities": bundle.missing_required_modalities,
        "alignment_order": bundle.alignment_order,
        "blocked_order": bundle.blocked_order,
        "omission_order": bundle.omission_order,
        "negative_evidence": bundle.negative_evidence,
        "limitations": bundle.limitations,
        "disposition": bundle.disposition,
    })
}

impl MultimodalResearchObjectBundle {
    pub fn validate(&self) -> Result<(), MultimodalResearchObjectError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.entries.is_empty()
            || !canonical(&self.modality_order)
            || !canonical(&self.covered_required_modalities)
            || !canonical(&self.missing_required_modalities)
            || !canonical(&self.alignment_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.limitations)
            || self.entries.iter().any(|entry| {
                !valid_text(&entry.artifact_id)
                    || !valid_text(&entry.source_program)
                    || !valid_text(&entry.schema_version)
                    || entry.semantic_loss_milli > 1_000
                    || entry.content_hash.as_str().len() != 64
                    || entry.provenance_digest.as_str().len() != 64
                    || !canonical(&entry.upstream_artifact_ids)
            })
        {
            return Err(MultimodalResearchObjectError::InvalidOutput(
                "identity, modality/entry ordering, semantic-loss bounds, provenance, or digest shape is invalid".into(),
            ));
        }
        self.manifest
            .validate()
            .map_err(|error| MultimodalResearchObjectError::InvalidManifest(error.to_string()))?;
        let entry_ids = self
            .entries
            .iter()
            .map(|entry| entry.artifact_id.clone())
            .collect::<BTreeSet<_>>();
        if entry_ids.len() != self.entries.len()
            || self
                .entries
                .iter()
                .any(|entry| !self.modality_order.contains(&entry.modality))
        {
            return Err(MultimodalResearchObjectError::InvalidOutput(
                "entry identities or modality partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultimodalResearchObjectError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultimodalResearchObjectError::Digest(
                "multimodal research-object digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MultimodalResearchObjectRequest,
) -> Result<(), MultimodalResearchObjectError> {
    if request.inputs.is_empty()
        || request.inputs.len() > MAX_INPUTS
        || request.max_inputs == 0
        || request.max_inputs > MAX_INPUTS
        || request.inputs.len() > request.max_inputs
        || request.max_semantic_loss_milli > 1_000
        || request.required_modalities.is_empty()
    {
        return Err(MultimodalResearchObjectError::InvalidRequest(
            "bounded inputs, required modalities, and semantic-loss limits are required".into(),
        ));
    }
    let mut artifact_ids = BTreeSet::new();
    for input in &request.inputs {
        if !artifact_ids.insert(input.artifact.artifact_id.clone())
            || input.artifact.validate().is_err()
            || !input.artifact.local_only
            || !valid_text(&input.source_program)
            || !valid_text(&input.schema_version)
            || input.semantic_loss_milli > 1_000
            || input.provenance_digest.as_str().len() != 64
            || input
                .upstream_artifact_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(MultimodalResearchObjectError::InvalidRequest(
                "artifact identity, local-only provenance, schema, semantic-loss, and upstream ordering are required".into(),
            ));
        }
    }
    Ok(())
}

/// Compile modality-specific artifacts into a release-ready, provenance-closed research object.
pub fn compile_glioma_multimodal_research_object(
    request: &MultimodalResearchObjectRequest,
) -> Result<MultimodalResearchObjectBundle, MultimodalResearchObjectError> {
    validate_request(request)?;
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut limitations = request
        .release
        .limitations
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let input_ids = request
        .inputs
        .iter()
        .map(|input| input.artifact.artifact_id.clone())
        .collect::<BTreeSet<_>>();
    let mut by_modality = BTreeMap::<GliomaModality, Vec<&MultimodalResearchObjectInput>>::new();
    for input in &request.inputs {
        by_modality.entry(input.modality).or_default().push(input);
        if input.semantic_loss_milli > request.max_semantic_loss_milli {
            blocked.insert(format!(
                "semantic-loss:{}>{}",
                input.artifact.artifact_id, request.max_semantic_loss_milli
            ));
        }
        for upstream in &input.upstream_artifact_ids {
            if !input_ids.contains(upstream) {
                blocked.insert(format!(
                    "missing-upstream:{}>{upstream}",
                    input.artifact.artifact_id
                ));
            }
        }
    }
    let mut covered = request
        .required_modalities
        .iter()
        .filter(|modality| by_modality.contains_key(modality))
        .copied()
        .collect::<Vec<_>>();
    covered.sort();
    let missing = request
        .required_modalities
        .iter()
        .filter(|modality| !by_modality.contains_key(modality))
        .copied()
        .collect::<Vec<_>>();
    for modality in &missing {
        omissions.insert(format!("missing-required-modality:{modality:?}"));
        blocked.insert(format!("required-modality-unavailable:{modality:?}"));
    }
    if request.require_cross_modal_alignment && covered.len() < 2 {
        blocked.insert("cross-modal-alignment-requires-two-covered-modalities".into());
        limitations.insert("cross-modal-alignment-unresolved".into());
    }
    let mut entries = request
        .inputs
        .iter()
        .map(|input| MultimodalResearchObjectEntry {
            modality: input.modality,
            artifact_id: input.artifact.artifact_id.clone(),
            content_hash: input.artifact.content_hash.clone(),
            source_program: input.source_program.clone(),
            schema_version: input.schema_version.clone(),
            semantic_loss_milli: input.semantic_loss_milli,
            provenance_digest: input.provenance_digest.clone(),
            upstream_artifact_ids: input.upstream_artifact_ids.clone(),
            required: input.required,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.modality
            .cmp(&right.modality)
            .then_with(|| left.artifact_id.cmp(&right.artifact_id))
    });
    let modality_order = by_modality.keys().copied().collect::<Vec<_>>();
    let mut alignment = BTreeSet::new();
    if request.require_cross_modal_alignment && blocked.is_empty() {
        for left_index in 0..covered.len() {
            for right_index in (left_index + 1)..covered.len() {
                let left = covered[left_index];
                let right = covered[right_index];
                let left_digest = &by_modality[&left][0].provenance_digest;
                let right_digest = &by_modality[&right][0].provenance_digest;
                if left_digest == right_digest {
                    alignment.insert(format!("{left:?}<->{right:?}:shared-provenance"));
                } else {
                    blocked.insert(format!(
                        "alignment-provenance-mismatch:{left:?}<->{right:?}"
                    ));
                }
            }
        }
    }
    if request.require_cross_modal_alignment && alignment.is_empty() {
        limitations.insert("cross-modal-alignment-not-proven".into());
    }
    let mut release = request.release.clone();
    release.artifacts = request
        .inputs
        .iter()
        .map(|input| input.artifact.clone())
        .collect();
    release.limitations = limitations.iter().cloned().collect();
    let manifest = build_research_object_manifest(&release)
        .map_err(|error| MultimodalResearchObjectError::InvalidManifest(error.to_string()))?;
    if manifest.release_status == ReleaseStatus::Blocked {
        blocked.extend(manifest.blocked_order.iter().cloned());
    }
    let negative_evidence = {
        let mut values = request.release.negative_evidence.clone();
        values.sort();
        values
    };
    let disposition = if !blocked.is_empty() {
        MultimodalResearchObjectDisposition::Blocked
    } else if !omissions.is_empty() || !negative_evidence.is_empty() {
        MultimodalResearchObjectDisposition::Partial
    } else {
        MultimodalResearchObjectDisposition::ReadyForSigning
    };
    let mut bundle = MultimodalResearchObjectBundle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        manifest,
        modality_order,
        entries,
        covered_required_modalities: covered,
        missing_required_modalities: missing,
        alignment_order: alignment.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        negative_evidence,
        limitations: limitations.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    bundle.digest = ContentHash::of_value(&digest_input(&bundle))
        .map_err(|error| MultimodalResearchObjectError::Digest(error.to_string()))?;
    bundle.validate()?;
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: "application/vnd.aurora.glioma.bundle+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn release() -> ResearchObjectRequest {
        ResearchObjectRequest {
            research_id: "research-1".into(),
            study_id: "study-1".into(),
            objective: "multimodal release".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["P03".into(), "P09".into(), "P10".into()],
            artifacts: vec![artifact("placeholder")],
            negative_evidence: Vec::new(),
            limitations: vec!["preclinical-only".into()],
            raw_data_local: true,
            aggregate_only: true,
        }
    }

    fn input(
        modality: GliomaModality,
        id: &str,
        provenance: &str,
    ) -> MultimodalResearchObjectInput {
        MultimodalResearchObjectInput {
            modality,
            artifact: artifact(id),
            source_program: "P03".into(),
            schema_version: "1.0".into(),
            semantic_loss_milli: 100,
            provenance_digest: hash(provenance),
            upstream_artifact_ids: Vec::new(),
            required: true,
        }
    }

    #[test]
    fn bundle_requires_shared_provenance_for_alignment() {
        let output = compile_glioma_multimodal_research_object(&MultimodalResearchObjectRequest {
            release: release(),
            inputs: vec![
                input(GliomaModality::Imaging, "imaging", "shared"),
                input(GliomaModality::Genomics, "genomics", "shared"),
            ],
            required_modalities: BTreeSet::from([
                GliomaModality::Imaging,
                GliomaModality::Genomics,
            ]),
            max_semantic_loss_milli: 200,
            require_cross_modal_alignment: true,
            max_inputs: 8,
        })
        .unwrap();
        assert_eq!(
            output.disposition,
            MultimodalResearchObjectDisposition::ReadyForSigning
        );
        assert_eq!(output.alignment_order.len(), 1);
        output.validate().unwrap();
    }

    #[test]
    fn bundle_blocks_missing_required_modality_and_high_loss() {
        let mut high_loss = input(GliomaModality::Imaging, "imaging", "shared");
        high_loss.semantic_loss_milli = 900;
        let output = compile_glioma_multimodal_research_object(&MultimodalResearchObjectRequest {
            release: release(),
            inputs: vec![high_loss],
            required_modalities: BTreeSet::from([
                GliomaModality::Imaging,
                GliomaModality::Genomics,
            ]),
            max_semantic_loss_milli: 200,
            require_cross_modal_alignment: true,
            max_inputs: 8,
        })
        .unwrap();
        assert_eq!(
            output.disposition,
            MultimodalResearchObjectDisposition::Blocked
        );
        assert!(!output.missing_required_modalities.is_empty());
        assert!(output
            .blocked_order
            .iter()
            .any(|item| item.starts_with("semantic-loss:")));
    }
}
