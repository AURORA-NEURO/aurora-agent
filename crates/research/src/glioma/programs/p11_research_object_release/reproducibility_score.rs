//! Reproducibility-completeness scoring for preclinical glioma research objects.
//!
//! This score is an evidence accounting product, not a confidence heuristic. Every dimension
//! has an explicit typed evidence row; missing rows score zero and can hard-block release. The
//! leave-one-component-out sensitivity surface shows which required components carry the claim,
//! while replay mismatches, null outcomes, uncertainty, and lineage gaps remain visible.

use super::replay::{ReplayCampaign, ReplayCampaignDisposition};
use crate::glioma::release::{ReleaseStatus, ResearchObjectManifest};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaReproducibilityCompletenessProfile1@1";
pub const MAX_DIMENSIONS: usize = 16;
pub const MAX_EVIDENCE_ROWS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibilityDimension {
    DataScope,
    Code,
    Environment,
    Methods,
    Artifacts,
    Uncertainty,
    NegativeOutcomes,
    Lineage,
    IndependentReplay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityEvidence {
    pub dimension: ReproducibilityDimension,
    pub evidence_id: String,
    pub present: bool,
    pub quality_milli: u16,
    pub coverage_milli: u16,
    pub source_digest: ContentHash,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityCompletenessRequest {
    pub manifest: ResearchObjectManifest,
    pub evidence: Vec<ReproducibilityEvidence>,
    pub required_dimension_order: Vec<ReproducibilityDimension>,
    pub independent_replay_count: u16,
    pub minimum_score_milli: u16,
    pub minimum_independent_replays: u16,
    pub require_exact_replay: bool,
    pub require_negative_outcome_accounting: bool,
    pub require_uncertainty_accounting: bool,
    pub replay: Option<ReplayCampaign>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityDimensionScore {
    pub dimension: ReproducibilityDimension,
    pub required: bool,
    pub present: bool,
    pub score_milli: u16,
    pub evidence_order: Vec<String>,
    pub missing_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilitySensitivity {
    pub dimension: ReproducibilityDimension,
    pub score_without_milli: u16,
    pub delta_milli: i32,
    pub blocks_without_component: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibilityCompletenessDisposition {
    Complete,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityCompletenessProfile {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub manifest_digest: ContentHash,
    pub dimension_scores: Vec<ReproducibilityDimensionScore>,
    pub sensitivity: Vec<ReproducibilitySensitivity>,
    pub overall_score_milli: u16,
    pub independent_replay_count: u16,
    pub replay_disposition: Option<ReplayCampaignDisposition>,
    pub hard_blocker_order: Vec<String>,
    pub warning_order: Vec<String>,
    pub missing_component_order: Vec<ReproducibilityDimension>,
    pub disposition: ReproducibilityCompletenessDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReproducibilityCompletenessError {
    #[error("reproducibility completeness request is invalid: {0}")]
    InvalidRequest(String),
    #[error("reproducibility completeness output is invalid: {0}")]
    InvalidOutput(String),
    #[error("reproducibility completeness digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 512
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn body(profile: &ReproducibilityCompletenessProfile) -> serde_json::Value {
    serde_json::json!({
        "feature_id": profile.feature_id,
        "output_schema": profile.output_schema,
        "research_id": profile.research_id,
        "study_id": profile.study_id,
        "manifest_digest": profile.manifest_digest,
        "dimension_scores": profile.dimension_scores,
        "sensitivity": profile.sensitivity,
        "overall_score_milli": profile.overall_score_milli,
        "independent_replay_count": profile.independent_replay_count,
        "replay_disposition": profile.replay_disposition,
        "hard_blocker_order": profile.hard_blocker_order,
        "warning_order": profile.warning_order,
        "missing_component_order": profile.missing_component_order,
        "disposition": profile.disposition,
    })
}

fn validate_request(
    request: &ReproducibilityCompletenessRequest,
) -> Result<(), ReproducibilityCompletenessError> {
    request
        .manifest
        .validate()
        .map_err(|error| ReproducibilityCompletenessError::InvalidRequest(error.to_string()))?;
    if request.evidence.is_empty()
        || request.evidence.len() > MAX_EVIDENCE_ROWS
        || request.required_dimension_order.is_empty()
        || request.required_dimension_order.len() > MAX_DIMENSIONS
        || !canonical(&request.required_dimension_order)
        || request.minimum_score_milli > 1_000
        || request.minimum_independent_replays == 0
    {
        return Err(ReproducibilityCompletenessError::InvalidRequest(
            "bounded evidence, required dimensions, score, and independent replay gates are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for evidence in &request.evidence {
        if !safe_text(&evidence.evidence_id)
            || !ids.insert(evidence.evidence_id.clone())
            || evidence.quality_milli > 1_000
            || evidence.coverage_milli > 1_000
            || !valid_hash(&evidence.source_digest)
            || !safe_text(&evidence.note)
        {
            return Err(ReproducibilityCompletenessError::InvalidRequest(
                "evidence rows require unique identity, bounded scores, source digest, and note"
                    .into(),
            ));
        }
    }
    if let Some(replay) = &request.replay {
        replay
            .validate()
            .map_err(|error| ReproducibilityCompletenessError::InvalidRequest(error.to_string()))?;
        if replay.manifest.manifest_digest != request.manifest.manifest_digest {
            return Err(ReproducibilityCompletenessError::InvalidRequest(
                "replay campaign manifest does not match the scored manifest".into(),
            ));
        }
    }
    Ok(())
}

impl ReproducibilityCompletenessProfile {
    pub fn validate(&self) -> Result<(), ReproducibilityCompletenessError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.research_id)
            || !safe_text(&self.study_id)
            || !valid_hash(&self.manifest_digest)
            || !canonical(&self.hard_blocker_order)
            || !canonical(&self.warning_order)
            || !canonical(&self.missing_component_order)
            || self.dimension_scores.len() > MAX_DIMENSIONS
            || self.sensitivity.len() != self.dimension_scores.len()
            || self.dimension_scores.iter().any(|score| {
                score.score_milli > 1_000
                    || score
                        .evidence_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
            })
            || self.sensitivity.iter().any(|sensitivity| {
                sensitivity.score_without_milli > 1_000
                    || sensitivity.dimension == ReproducibilityDimension::IndependentReplay
                        && sensitivity.delta_milli < 0
                        && !sensitivity.blocks_without_component
            })
            || self.overall_score_milli > 1_000
        {
            return Err(ReproducibilityCompletenessError::InvalidOutput(
                "profile identity, dimensions, sensitivity, ordering, or score bounds are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&body(self))
            .map_err(|error| ReproducibilityCompletenessError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReproducibilityCompletenessError::InvalidOutput(
                "profile digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Score whether a preclinical research object contains enough explicit evidence for replay.
pub fn score_glioma_reproducibility_completeness(
    request: &ReproducibilityCompletenessRequest,
) -> Result<ReproducibilityCompletenessProfile, ReproducibilityCompletenessError> {
    validate_request(request)?;
    let required = request
        .required_dimension_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut grouped = BTreeMap::<ReproducibilityDimension, Vec<&ReproducibilityEvidence>>::new();
    for evidence in &request.evidence {
        grouped
            .entry(evidence.dimension)
            .or_default()
            .push(evidence);
    }
    let mut scores = Vec::new();
    let mut missing = Vec::new();
    let mut blockers = BTreeSet::new();
    let mut warnings = BTreeSet::new();
    for dimension in &request.required_dimension_order {
        let rows = grouped.get(dimension).cloned().unwrap_or_default();
        let present_rows = rows.iter().filter(|row| row.present).collect::<Vec<_>>();
        let score = if present_rows.is_empty() {
            0
        } else {
            let total = present_rows
                .iter()
                .map(|row| u32::from(row.quality_milli) * u32::from(row.coverage_milli) / 1_000)
                .sum::<u32>();
            (total / present_rows.len() as u32).min(1_000) as u16
        };
        let evidence_order = rows
            .iter()
            .map(|row| row.evidence_id.clone())
            .collect::<Vec<_>>();
        let present = !present_rows.is_empty();
        let missing_reason = if present {
            None
        } else {
            let reason = format!("{dimension:?} has no explicit present evidence; score is zero");
            missing.push(*dimension);
            blockers.insert(format!("missing:{dimension:?}"));
            Some(reason)
        };
        if present && score < request.minimum_score_milli {
            blockers.insert(format!("below-threshold:{dimension:?}:{score}"));
        }
        if rows.iter().any(|row| !row.present) {
            warnings.insert(format!("negative-or-missing-evidence:{dimension:?}"));
        }
        scores.push(ReproducibilityDimensionScore {
            dimension: *dimension,
            required: true,
            present,
            score_milli: score,
            evidence_order,
            missing_reason,
        });
    }
    if request.manifest.release_status == ReleaseStatus::Blocked {
        blockers.insert("manifest-blocked".into());
    }
    if request.require_negative_outcome_accounting
        && !required.contains(&ReproducibilityDimension::NegativeOutcomes)
    {
        blockers.insert("negative-outcome-dimension-not-required".into());
    }
    if request.require_uncertainty_accounting
        && !required.contains(&ReproducibilityDimension::Uncertainty)
    {
        blockers.insert("uncertainty-dimension-not-required".into());
    }
    let replay_disposition = request.replay.as_ref().map(|replay| replay.disposition);
    if request.independent_replay_count < request.minimum_independent_replays {
        blockers.insert(format!(
            "independent-replays-below-gate:{}<{}",
            request.independent_replay_count, request.minimum_independent_replays
        ));
    }
    if request.require_exact_replay
        && request
            .replay
            .as_ref()
            .is_none_or(|replay| !replay.exact_match)
    {
        blockers.insert("exact-replay-required".into());
    }
    if request
        .replay
        .as_ref()
        .is_some_and(|replay| replay.disposition != ReplayCampaignDisposition::Reproducible)
    {
        blockers.insert("replay-not-reproducible".into());
    }
    let total_weight = scores.len() as u32;
    let overall_score = if total_weight == 0 {
        0
    } else {
        (scores
            .iter()
            .map(|score| u32::from(score.score_milli))
            .sum::<u32>()
            .checked_div(total_weight)
            .unwrap_or(0)) as u16
    };
    let mut sensitivity = Vec::new();
    for score in &scores {
        let remaining = scores.len().saturating_sub(1) as u32;
        let without = if remaining == 0 {
            0
        } else {
            ((scores
                .iter()
                .filter(|other| other.dimension != score.dimension)
                .map(|other| u32::from(other.score_milli))
                .sum::<u32>()
                .checked_div(remaining)
                .unwrap_or(0))
            .min(1_000)) as u16
        };
        sensitivity.push(ReproducibilitySensitivity {
            dimension: score.dimension,
            score_without_milli: without,
            delta_milli: i32::from(overall_score) - i32::from(without),
            blocks_without_component: score.required
                && (!score.present || score.score_milli < request.minimum_score_milli),
        });
    }
    let mut profile = ReproducibilityCompletenessProfile {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.manifest.research_id.clone(),
        study_id: request.manifest.study_id.clone(),
        manifest_digest: request.manifest.manifest_digest.clone(),
        dimension_scores: scores,
        sensitivity,
        overall_score_milli: overall_score,
        independent_replay_count: request.independent_replay_count,
        replay_disposition,
        hard_blocker_order: blockers.into_iter().collect(),
        warning_order: warnings.into_iter().collect(),
        missing_component_order: missing,
        disposition: ReproducibilityCompletenessDisposition::Unresolved,
        digest: ContentHash::of_bytes(b"unsealed-glioma-reproducibility-profile"),
    };
    profile.disposition = if !profile.hard_blocker_order.is_empty() {
        ReproducibilityCompletenessDisposition::Blocked
    } else if profile.overall_score_milli < request.minimum_score_milli
        || !profile.warning_order.is_empty()
    {
        ReproducibilityCompletenessDisposition::Partial
    } else {
        ReproducibilityCompletenessDisposition::Complete
    };
    profile.digest = ContentHash::of_value(&body(&profile))
        .map_err(|error| ReproducibilityCompletenessError::Digest(error.to_string()))?;
    profile.validate()?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::{build_research_object_manifest, ResearchObjectRequest};
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn manifest() -> ResearchObjectManifest {
        build_research_object_manifest(&ResearchObjectRequest {
            research_id: "research-1".into(),
            study_id: "study-1".into(),
            objective: "replay a preclinical glioma result".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["p05".into(), "p10".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact-1".into(),
                content_hash: hash("artifact"),
                content_type: "application/vnd.aurora.glioma+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: vec!["null-result".into()],
            limitations: vec!["single-model".into()],
            raw_data_local: true,
            aggregate_only: true,
        })
        .unwrap()
    }

    fn evidence(all: bool) -> Vec<ReproducibilityEvidence> {
        let dimensions = [
            ReproducibilityDimension::DataScope,
            ReproducibilityDimension::Code,
            ReproducibilityDimension::Environment,
            ReproducibilityDimension::Methods,
            ReproducibilityDimension::Artifacts,
            ReproducibilityDimension::Uncertainty,
            ReproducibilityDimension::NegativeOutcomes,
            ReproducibilityDimension::Lineage,
            ReproducibilityDimension::IndependentReplay,
        ];
        dimensions
            .iter()
            .enumerate()
            .map(|(index, dimension)| ReproducibilityEvidence {
                dimension: *dimension,
                evidence_id: format!("e-{index}"),
                present: all,
                quality_milli: 950,
                coverage_milli: 950,
                source_digest: hash(&format!("source-{index}")),
                note: "explicit local release evidence".into(),
            })
            .collect()
    }

    fn request(all: bool) -> ReproducibilityCompletenessRequest {
        ReproducibilityCompletenessRequest {
            manifest: manifest(),
            evidence: evidence(all),
            required_dimension_order: vec![
                ReproducibilityDimension::DataScope,
                ReproducibilityDimension::Code,
                ReproducibilityDimension::Environment,
                ReproducibilityDimension::Methods,
                ReproducibilityDimension::Artifacts,
                ReproducibilityDimension::Uncertainty,
                ReproducibilityDimension::NegativeOutcomes,
                ReproducibilityDimension::Lineage,
                ReproducibilityDimension::IndependentReplay,
            ],
            independent_replay_count: 2,
            minimum_score_milli: 800,
            minimum_independent_replays: 2,
            require_exact_replay: false,
            require_negative_outcome_accounting: true,
            require_uncertainty_accounting: true,
            replay: None,
        }
    }

    #[test]
    fn complete_explicit_dimensions_produce_complete_profile() {
        let profile = score_glioma_reproducibility_completeness(&request(true)).unwrap();
        assert_eq!(
            profile.disposition,
            ReproducibilityCompletenessDisposition::Complete
        );
        assert!(profile.overall_score_milli >= 900);
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn absent_evidence_is_zero_and_blocks_instead_of_imputation() {
        let profile = score_glioma_reproducibility_completeness(&request(false)).unwrap();
        assert_eq!(
            profile.disposition,
            ReproducibilityCompletenessDisposition::Blocked
        );
        assert_eq!(profile.overall_score_milli, 0);
        assert_eq!(profile.missing_component_order.len(), 9);
    }

    #[test]
    fn removing_required_dimension_changes_sensitivity_surface() {
        let mut req = request(true);
        req.evidence
            .retain(|row| row.dimension != ReproducibilityDimension::Environment);
        let profile = score_glioma_reproducibility_completeness(&req).unwrap();
        let sensitivity = profile
            .sensitivity
            .iter()
            .find(|sensitivity| sensitivity.dimension == ReproducibilityDimension::Environment)
            .unwrap();
        assert!(sensitivity.blocks_without_component);
        assert!(profile
            .hard_blocker_order
            .iter()
            .any(|item| item.contains("Environment")));
    }

    #[test]
    fn exact_replay_gate_requires_a_replay_campaign() {
        let mut req = request(true);
        req.require_exact_replay = true;
        let profile = score_glioma_reproducibility_completeness(&req).unwrap();
        assert!(profile
            .hard_blocker_order
            .iter()
            .any(|item| item == "exact-replay-required"));
    }

    #[test]
    fn negative_and_uncertainty_dimensions_cannot_be_omitted() {
        let mut req = request(true);
        req.required_dimension_order = vec![ReproducibilityDimension::DataScope];
        let profile = score_glioma_reproducibility_completeness(&req).unwrap();
        assert!(profile
            .hard_blocker_order
            .iter()
            .any(|item| item.contains("negative-outcome")));
        assert!(profile
            .hard_blocker_order
            .iter()
            .any(|item| item.contains("uncertainty")));
    }
}
