//! Cross-study replication, robustness, and negative-result assessment.
//!
//! Replication is evaluated across preclinical studies, not individual patients.  The assessment
//! keeps site-level disagreement and insufficient coverage explicit; a concordant direction alone
//! never turns an underpowered or heterogeneous portfolio into a pass.

use super::super::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationAssessment1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationStudy {
    pub study_id: String,
    pub site_id: String,
    pub model_system: GliomaModelSystem,
    pub artifact: LocalArtifactRef,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub replicate_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub min_sites: usize,
    pub min_replicates_per_site: u16,
    pub effect_threshold_milli: u64,
    pub heterogeneity_tolerance_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationDisposition {
    Replicated,
    Mixed,
    NotReplicated,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationAssessment {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_order: Vec<String>,
    pub site_order: Vec<String>,
    pub pooled_effect_milli: i64,
    pub heterogeneity_milli: u64,
    pub concordant_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub insufficient_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ReplicationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationError {
    #[error("replication request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication study is invalid: {0}")]
    InvalidStudy(String),
    #[error("replication assessment is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication digest failed: {0}")]
    Digest(String),
}

fn sign(value: i64) -> i8 {
    if value > 0 {
        1
    } else if value < 0 {
        -1
    } else {
        0
    }
}

fn digest_input(assessment: &ReplicationAssessment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": assessment.feature_id,
        "output_schema": assessment.output_schema,
        "objective": assessment.objective,
        "study_order": assessment.study_order,
        "site_order": assessment.site_order,
        "pooled_effect_milli": assessment.pooled_effect_milli,
        "heterogeneity_milli": assessment.heterogeneity_milli,
        "concordant_order": assessment.concordant_order,
        "contradictory_order": assessment.contradictory_order,
        "insufficient_order": assessment.insufficient_order,
        "negative_evidence": assessment.negative_evidence,
        "uncertainty": assessment.uncertainty,
        "disposition": assessment.disposition,
    })
}

impl ReplicationAssessment {
    pub fn validate(&self) -> Result<(), ReplicationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.study_order.windows(2).any(|pair| pair[0] > pair[1])
            || self.site_order.windows(2).any(|pair| pair[0] > pair[1])
            || self
                .concordant_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .contradictory_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .insufficient_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] > pair[1])
        {
            return Err(ReplicationError::InvalidOutput(
                "identity or canonical ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| ReplicationError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(ReplicationError::InvalidOutput(
                "digest is not bound to the replication assessment".into(),
            ));
        }
        Ok(())
    }
}

pub fn assess_replication(
    request: &ReplicationRequest,
    studies: &[ReplicationStudy],
) -> Result<ReplicationAssessment, ReplicationError> {
    if request.objective.trim().is_empty()
        || request.min_sites == 0
        || request.min_replicates_per_site == 0
        || request.effect_threshold_milli == 0
    {
        return Err(ReplicationError::InvalidRequest(
            "objective, site floor, replicate floor, and effect threshold are required".into(),
        ));
    }
    let mut study_ids = BTreeSet::new();
    let mut site_ids = BTreeSet::new();
    for study in studies {
        study
            .artifact
            .validate()
            .map_err(|e| ReplicationError::InvalidStudy(e.to_string()))?;
        if study.study_id.trim().is_empty()
            || study.site_id.trim().is_empty()
            || study.model_system != request.model_system
            || study.replicate_count == 0
            || !study_ids.insert(study.study_id.clone())
            || !site_ids.insert(study.site_id.clone())
        {
            return Err(ReplicationError::InvalidStudy(
                "study/site identity, model binding, replicate count, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    let mut insufficient_order = studies
        .iter()
        .filter(|study| study.replicate_count < request.min_replicates_per_site)
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    insufficient_order.sort();
    let eligible = studies
        .iter()
        .filter(|study| !insufficient_order.contains(&study.study_id))
        .collect::<Vec<_>>();
    let mut concordant_order = Vec::new();
    let mut contradictory_order = Vec::new();
    let reference_sign = eligible
        .first()
        .map(|study| sign(study.effect_milli))
        .unwrap_or(0);
    for study in &eligible {
        if reference_sign != 0
            && sign(study.effect_milli) == reference_sign
            && study.effect_milli.unsigned_abs() >= request.effect_threshold_milli
        {
            concordant_order.push(study.study_id.clone());
        } else if reference_sign != 0
            && sign(study.effect_milli) != 0
            && sign(study.effect_milli) != reference_sign
        {
            contradictory_order.push(study.study_id.clone());
        }
    }
    concordant_order.sort();
    contradictory_order.sort();
    let total_weight = eligible
        .iter()
        .map(|study| study.replicate_count as u64)
        .sum::<u64>();
    let pooled_effect_milli = if total_weight == 0 {
        0
    } else {
        eligible
            .iter()
            .map(|study| study.effect_milli as i128 * study.replicate_count as i128)
            .sum::<i128>()
            / total_weight as i128
    } as i64;
    let (min_effect, max_effect) =
        eligible
            .iter()
            .map(|study| study.effect_milli)
            .fold((None, None), |(min, max), effect| {
                (
                    Some(min.map_or(effect, |value: i64| value.min(effect))),
                    Some(max.map_or(effect, |value: i64| value.max(effect))),
                )
            });
    let heterogeneity_milli = match (min_effect, max_effect) {
        (Some(min), Some(max)) => max.saturating_sub(min).unsigned_abs(),
        _ => 0,
    };
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if studies.len() < request.min_sites {
        negative.insert("minimum-site-count-not-met".into());
    }
    if !insufficient_order.is_empty() {
        uncertainty.insert("one-or-more-sites-below-replicate-floor".into());
    }
    if !contradictory_order.is_empty() {
        negative.insert("cross-site-direction-contradiction".into());
    }
    if heterogeneity_milli > request.heterogeneity_tolerance_milli {
        uncertainty.insert("effect-heterogeneity-exceeds-tolerance".into());
    }
    let disposition = if studies.len() < request.min_sites || eligible.len() < request.min_sites {
        ReplicationDisposition::Unresolved
    } else if !contradictory_order.is_empty() {
        ReplicationDisposition::Mixed
    } else if concordant_order.len() >= request.min_sites
        && heterogeneity_milli <= request.heterogeneity_tolerance_milli
    {
        ReplicationDisposition::Replicated
    } else {
        negative.insert("declared-effect-not-replicated".into());
        ReplicationDisposition::NotReplicated
    };
    let mut assessment = ReplicationAssessment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_order: studies.iter().map(|study| study.study_id.clone()).collect(),
        site_order: site_ids.into_iter().collect(),
        pooled_effect_milli,
        heterogeneity_milli,
        concordant_order,
        contradictory_order,
        insufficient_order,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| ReplicationError::Digest(e.to_string()))?,
    };
    assessment.study_order.sort();
    assessment.digest = ContentHash::of_value(&digest_input(&assessment))
        .map_err(|e| ReplicationError::Digest(e.to_string()))?;
    assessment.validate()?;
    Ok(assessment)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn study(id: &str, site: &str, effect: i64, replicates: u16) -> ReplicationStudy {
        ReplicationStudy {
            study_id: id.into(),
            site_id: site.into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-replication+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            effect_milli: effect,
            uncertainty_milli: 20,
            replicate_count: replicates,
        }
    }

    fn request() -> ReplicationRequest {
        ReplicationRequest {
            objective: "replicate invasion effect".into(),
            model_system: GliomaModelSystem::Organoid,
            min_sites: 3,
            min_replicates_per_site: 3,
            effect_threshold_milli: 100,
            heterogeneity_tolerance_milli: 80,
        }
    }

    #[test]
    fn concordant_multi_site_effect_is_replicated() {
        let assessment = assess_replication(
            &request(),
            &[
                study("s1", "site-a", 200, 4),
                study("s2", "site-b", 220, 4),
                study("s3", "site-c", 180, 4),
            ],
        )
        .unwrap();
        assert_eq!(assessment.disposition, ReplicationDisposition::Replicated);
        assert_eq!(assessment.concordant_order.len(), 3);
        assessment.validate().unwrap();
    }

    #[test]
    fn contradictory_site_is_mixed_and_not_hidden() {
        let assessment = assess_replication(
            &request(),
            &[
                study("s1", "site-a", 200, 4),
                study("s2", "site-b", -180, 4),
                study("s3", "site-c", 190, 4),
            ],
        )
        .unwrap();
        assert_eq!(assessment.disposition, ReplicationDisposition::Mixed);
        assert_eq!(assessment.contradictory_order, vec!["s2"]);
        assert!(assessment
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradiction")));
    }
}
