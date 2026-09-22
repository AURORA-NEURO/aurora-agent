//! Deterministic fixed-point meta-analysis for independent preclinical glioma studies.
//!
//! Each study contributes an effect and a declared uncertainty in milli-units. Fixed-effect and
//! deterministic random-effects inverse-variance pools, a Cochran-style heterogeneity statistic,
//! an I² estimate, and leave-one-study-out influence are computed with bounded integer arithmetic.
//! The output is an analysis product for research replication; it never treats a pooled effect as
//! a clinical recommendation and never hides contradictory, underpowered, or unstable studies.

use crate::glioma::replication::ReplicationStudy;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationMetaAnalysis1@1";
pub const MAX_STUDIES: usize = 4_096;
pub const MAX_EFFECT_MILLI: u64 = 1_000_000_000;
const WEIGHT_SCALE: u128 = 1_000_000_000_000;
const MAX_VARIANCE_MILLI: u128 = (MAX_EFFECT_MILLI as u128) * (MAX_EFFECT_MILLI as u128);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaAnalysisRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub min_studies: usize,
    pub min_replicates_per_study: u16,
    pub effect_threshold_milli: u64,
    pub max_i2_milli: u16,
    pub min_signal_to_noise_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaStudyContribution {
    pub study_id: String,
    pub weight_milli: u64,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetaAnalysisDisposition {
    Qualified,
    Heterogeneous,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationMetaAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_order: Vec<String>,
    pub included_order: Vec<String>,
    pub excluded_order: Vec<String>,
    pub contributions: Vec<MetaStudyContribution>,
    pub pooled_effect_milli: i64,
    pub pooled_uncertainty_milli: u64,
    pub signal_to_noise_milli: u64,
    pub random_effect_milli: i64,
    pub random_effect_uncertainty_milli: u64,
    pub random_signal_to_noise_milli: u64,
    pub between_study_variance_milli: u64,
    pub cochran_q_milli: u64,
    pub i2_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MetaAnalysisDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetaAnalysisError {
    #[error("meta-analysis request is invalid: {0}")]
    InvalidRequest(String),
    #[error("meta-analysis study is invalid: {0}")]
    InvalidStudy(String),
    #[error("meta-analysis output is invalid: {0}")]
    InvalidOutput(String),
    #[error("meta-analysis digest failed: {0}")]
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

fn weight_for(uncertainty_milli: u64) -> u128 {
    let variance = u128::from(uncertainty_milli).saturating_mul(u128::from(uncertainty_milli));
    WEIGHT_SCALE / variance.max(1)
}

fn weighted_effect(studies: &[&ReplicationStudy]) -> (i64, u128) {
    let total_weight = studies
        .iter()
        .map(|study| weight_for(study.uncertainty_milli))
        .sum::<u128>();
    if total_weight == 0 {
        return (0, 0);
    }
    let numerator = studies
        .iter()
        .map(|study| {
            i128::from(study.effect_milli)
                .saturating_mul(weight_for(study.uncertainty_milli) as i128)
        })
        .sum::<i128>();
    (
        (numerator / total_weight as i128).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
        total_weight,
    )
}

fn weight_for_tau(uncertainty_milli: u64, tau_squared_milli: u64) -> u128 {
    let variance = u128::from(uncertainty_milli)
        .saturating_mul(u128::from(uncertainty_milli))
        .saturating_add(u128::from(tau_squared_milli));
    WEIGHT_SCALE / variance.max(1)
}

fn weighted_effect_with_tau(studies: &[&ReplicationStudy], tau_squared_milli: u64) -> (i64, u128) {
    let total_weight = studies
        .iter()
        .map(|study| weight_for_tau(study.uncertainty_milli, tau_squared_milli))
        .sum::<u128>();
    if total_weight == 0 {
        return (0, 0);
    }
    let numerator = studies
        .iter()
        .map(|study| {
            i128::from(study.effect_milli)
                .saturating_mul(weight_for_tau(study.uncertainty_milli, tau_squared_milli) as i128)
        })
        .sum::<i128>();
    (
        (numerator / total_weight as i128).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
        total_weight,
    )
}

/// Estimate DerSimonian–Laird-style between-study variance in fixed-point milli² units.
/// `cochran_q_milli` and degrees of freedom are represented at a 1/1000 scale and all divisions
/// are integer-only, so replaying on another supported language yields the same bytes.
fn estimate_between_study_variance(studies: &[&ReplicationStudy], cochran_q_milli: u64) -> u64 {
    if studies.len() < 2 {
        return 0;
    }
    let degrees_of_freedom_milli = (studies.len() - 1) as u64 * 1_000;
    if cochran_q_milli <= degrees_of_freedom_milli {
        return 0;
    }
    let total_weight = studies
        .iter()
        .map(|study| weight_for(study.uncertainty_milli))
        .sum::<u128>();
    if total_weight == 0 {
        return 0;
    }
    let sum_squared_weights = studies
        .iter()
        .map(|study| {
            let weight = weight_for(study.uncertainty_milli);
            weight.saturating_mul(weight)
        })
        .sum::<u128>();
    let correction = sum_squared_weights / total_weight;
    let c = total_weight.saturating_sub(correction);
    if c == 0 {
        return 0;
    }
    let excess_q_milli = u128::from(cochran_q_milli - degrees_of_freedom_milli);
    excess_q_milli
        .saturating_mul(WEIGHT_SCALE)
        .checked_div(c.saturating_mul(1_000))
        .unwrap_or(0)
        .min(MAX_VARIANCE_MILLI) as u64
}

fn pooled_uncertainty(total_weight: u128) -> u64 {
    if total_weight == 0 {
        return 0;
    }
    let variance = WEIGHT_SCALE / total_weight;
    integer_sqrt(variance).min(u128::from(u64::MAX)) as u64
}

fn integer_sqrt(value: u128) -> u128 {
    let mut low = 0_u128;
    let mut high = value.saturating_add(1);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid.max(1) {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

fn digest_input(output: &ReplicationMetaAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_order": output.study_order,
        "included_order": output.included_order,
        "excluded_order": output.excluded_order,
        "contributions": output.contributions,
        "pooled_effect_milli": output.pooled_effect_milli,
        "pooled_uncertainty_milli": output.pooled_uncertainty_milli,
        "signal_to_noise_milli": output.signal_to_noise_milli,
        "random_effect_milli": output.random_effect_milli,
        "random_effect_uncertainty_milli": output.random_effect_uncertainty_milli,
        "random_signal_to_noise_milli": output.random_signal_to_noise_milli,
        "between_study_variance_milli": output.between_study_variance_milli,
        "cochran_q_milli": output.cochran_q_milli,
        "i2_milli": output.i2_milli,
        "max_leave_one_out_shift_milli": output.max_leave_one_out_shift_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl ReplicationMetaAnalysis {
    pub fn validate(&self) -> Result<(), MetaAnalysisError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.i2_milli > 1_000
            || self.study_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .included_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .excluded_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .contributions
                .windows(2)
                .any(|pair| pair[0].study_id >= pair[1].study_id)
            || self.pooled_effect_milli.unsigned_abs() > MAX_EFFECT_MILLI
            || self.random_effect_milli.unsigned_abs() > MAX_EFFECT_MILLI
            || self.pooled_uncertainty_milli > MAX_EFFECT_MILLI
            || self.random_effect_uncertainty_milli > MAX_EFFECT_MILLI
            || u128::from(self.between_study_variance_milli) > MAX_VARIANCE_MILLI
            || self.contributions.iter().any(|item| {
                item.study_id.trim().is_empty()
                    || item.weight_milli == 0
                    || item.uncertainty_milli == 0
                    || item.effect_milli.unsigned_abs() > MAX_EFFECT_MILLI
            })
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(MetaAnalysisError::InvalidOutput(
                "identity, bounds, uniqueness, or canonical ordering is invalid".into(),
            ));
        }
        let study_ids = self.study_order.iter().cloned().collect::<BTreeSet<_>>();
        let included_ids = self.included_order.iter().cloned().collect::<BTreeSet<_>>();
        let excluded_ids = self.excluded_order.iter().cloned().collect::<BTreeSet<_>>();
        let contribution_ids = self
            .contributions
            .iter()
            .map(|item| item.study_id.clone())
            .collect::<BTreeSet<_>>();
        if included_ids.intersection(&excluded_ids).next().is_some()
            || included_ids
                .union(&excluded_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != study_ids
            || contribution_ids != included_ids
            || self.contributions.len() != contribution_ids.len()
        {
            return Err(MetaAnalysisError::InvalidOutput(
                "included, excluded, and contribution study partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MetaAnalysisError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MetaAnalysisError::InvalidOutput(
                "digest is not bound to the meta-analysis".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_replication_meta_analysis(
    request: &MetaAnalysisRequest,
    studies: &[ReplicationStudy],
) -> Result<ReplicationMetaAnalysis, MetaAnalysisError> {
    if request.objective.trim().is_empty()
        || request.min_studies == 0
        || request.min_replicates_per_study == 0
        || request.effect_threshold_milli == 0
        || request.max_i2_milli > 1_000
        || request.min_signal_to_noise_milli == 0
        || studies.len() > MAX_STUDIES
    {
        return Err(MetaAnalysisError::InvalidRequest(
            "objective, study/replicate floors, effect threshold, heterogeneity bound, signal floor, and study bound are required".into(),
        ));
    }
    let mut study_ids = BTreeSet::new();
    let mut site_ids = BTreeSet::new();
    for study in studies {
        study
            .artifact
            .validate()
            .map_err(|error| MetaAnalysisError::InvalidStudy(error.to_string()))?;
        if study.study_id.trim().is_empty()
            || study.site_id.trim().is_empty()
            || study.model_system != request.model_system
            || study.replicate_count == 0
            || study.uncertainty_milli == 0
            || study.uncertainty_milli > MAX_EFFECT_MILLI
            || study.effect_milli.unsigned_abs() > MAX_EFFECT_MILLI
            || !study_ids.insert(study.study_id.clone())
            || !site_ids.insert(study.site_id.clone())
        {
            return Err(MetaAnalysisError::InvalidStudy(
                "study/site identity, model binding, effect/uncertainty bounds, replicate count, or uniqueness is invalid".into(),
            ));
        }
    }
    let mut study_order = study_ids.iter().cloned().collect::<Vec<_>>();
    let mut included = studies
        .iter()
        .filter(|study| study.replicate_count >= request.min_replicates_per_study)
        .collect::<Vec<_>>();
    included.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let included_order = included
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let included_set = included_order.iter().cloned().collect::<BTreeSet<_>>();
    let excluded_order = study_order
        .iter()
        .filter(|study_id| !included_set.contains(*study_id))
        .cloned()
        .collect::<Vec<_>>();
    let (pooled_effect_milli, total_weight) = weighted_effect(&included);
    let pooled_uncertainty_milli = pooled_uncertainty(total_weight);
    let signal_to_noise_milli = if pooled_uncertainty_milli == 0 {
        0
    } else {
        pooled_effect_milli
            .unsigned_abs()
            .saturating_mul(1_000)
            .checked_div(pooled_uncertainty_milli)
            .unwrap_or(0)
    };
    let mut q_numerator = 0_u128;
    for study in &included {
        let delta = i128::from(study.effect_milli) - i128::from(pooled_effect_milli);
        q_numerator = q_numerator.saturating_add(
            weight_for(study.uncertainty_milli)
                .saturating_mul(delta.unsigned_abs().saturating_mul(delta.unsigned_abs())),
        );
    }
    let cochran_q_milli = q_numerator
        .saturating_mul(1_000)
        .checked_div(WEIGHT_SCALE)
        .unwrap_or(0)
        .min(u128::from(u64::MAX)) as u64;
    let degrees_of_freedom_milli = included.len().saturating_sub(1) as u64 * 1_000;
    let i2_milli = if cochran_q_milli <= degrees_of_freedom_milli || cochran_q_milli == 0 {
        0
    } else {
        (((cochran_q_milli - degrees_of_freedom_milli) as u128 * 1_000) / cochran_q_milli as u128)
            .min(1_000) as u16
    };
    let between_study_variance_milli = estimate_between_study_variance(&included, cochran_q_milli);
    let (random_effect_milli, random_total_weight) =
        weighted_effect_with_tau(&included, between_study_variance_milli);
    let random_effect_uncertainty_milli = pooled_uncertainty(random_total_weight);
    let random_signal_to_noise_milli = if random_effect_uncertainty_milli == 0 {
        0
    } else {
        random_effect_milli
            .unsigned_abs()
            .saturating_mul(1_000)
            .checked_div(random_effect_uncertainty_milli)
            .unwrap_or(0)
    };
    let mut max_leave_one_out_shift_milli = 0_u64;
    let mut contributions = Vec::with_capacity(included.len());
    for study in &included {
        let without = included
            .iter()
            .copied()
            .filter(|candidate| candidate.study_id != study.study_id)
            .collect::<Vec<_>>();
        let (leave_one_out_effect, _) =
            weighted_effect_with_tau(&without, between_study_variance_milli);
        let shift = random_effect_milli
            .saturating_sub(leave_one_out_effect)
            .unsigned_abs();
        max_leave_one_out_shift_milli = max_leave_one_out_shift_milli.max(shift);
        contributions.push(MetaStudyContribution {
            study_id: study.study_id.clone(),
            weight_milli: weight_for(study.uncertainty_milli).min(u128::from(u64::MAX)) as u64,
            effect_milli: study.effect_milli,
            uncertainty_milli: study.uncertainty_milli,
            leave_one_out_shift_milli: shift,
        });
    }
    contributions.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if studies.len() < request.min_studies || included.len() < request.min_studies {
        negative_evidence.insert("minimum-study-count-not-met".into());
    }
    if !excluded_order.is_empty() {
        uncertainty.insert("one-or-more-studies-below-replicate-floor".into());
    }
    let nonzero_signs = included
        .iter()
        .map(|study| sign(study.effect_milli))
        .filter(|direction| *direction != 0)
        .collect::<BTreeSet<_>>();
    if nonzero_signs.len() > 1 {
        negative_evidence.insert("cross-study-direction-contradiction".into());
    }
    if i2_milli > request.max_i2_milli {
        uncertainty.insert("effect-heterogeneity-exceeds-i2-tolerance".into());
    }
    if max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli {
        uncertainty.insert("pooled-effect-is-leave-one-study-sensitive".into());
    }
    if pooled_effect_milli.unsigned_abs() < request.effect_threshold_milli
        || signal_to_noise_milli < request.min_signal_to_noise_milli
        || random_effect_milli.unsigned_abs() < request.effect_threshold_milli
        || random_signal_to_noise_milli < request.min_signal_to_noise_milli
    {
        negative_evidence.insert("fixed-or-random-effect-does-not-clear-signal-threshold".into());
    }
    let disposition = if studies.len() < request.min_studies || included.len() < request.min_studies
    {
        MetaAnalysisDisposition::Unresolved
    } else if i2_milli > request.max_i2_milli {
        MetaAnalysisDisposition::Heterogeneous
    } else if !negative_evidence.is_empty() {
        MetaAnalysisDisposition::Negative
    } else if !uncertainty.is_empty() {
        MetaAnalysisDisposition::Unresolved
    } else {
        MetaAnalysisDisposition::Qualified
    };
    let mut output = ReplicationMetaAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_order: std::mem::take(&mut study_order),
        included_order,
        excluded_order,
        contributions,
        pooled_effect_milli,
        pooled_uncertainty_milli,
        signal_to_noise_milli,
        random_effect_milli,
        random_effect_uncertainty_milli,
        random_signal_to_noise_milli,
        between_study_variance_milli,
        cochran_q_milli,
        i2_milli,
        max_leave_one_out_shift_milli,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| MetaAnalysisError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MetaAnalysisError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn study(
        id: &str,
        site: &str,
        effect: i64,
        uncertainty: u64,
        replicates: u16,
    ) -> ReplicationStudy {
        ReplicationStudy {
            study_id: id.into(),
            site_id: site.into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-meta-study+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            effect_milli: effect,
            uncertainty_milli: uncertainty,
            replicate_count: replicates,
        }
    }

    fn request() -> MetaAnalysisRequest {
        MetaAnalysisRequest {
            objective: "pool invasion effects".into(),
            model_system: GliomaModelSystem::Organoid,
            min_studies: 3,
            min_replicates_per_study: 3,
            effect_threshold_milli: 100,
            max_i2_milli: 200,
            min_signal_to_noise_milli: 1_000,
            max_leave_one_out_shift_milli: 60,
        }
    }

    #[test]
    fn stable_multi_site_effect_is_qualified_and_replay_stable() {
        let studies = vec![
            study("s1", "site-a", 300, 20, 4),
            study("s2", "site-b", 310, 25, 4),
            study("s3", "site-c", 295, 22, 4),
        ];
        let first = analyze_replication_meta_analysis(&request(), &studies).unwrap();
        let second = analyze_replication_meta_analysis(&request(), &studies).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, MetaAnalysisDisposition::Qualified);
        assert!(first.i2_milli <= 200);
        assert_eq!(first.included_order.len(), 3);
        assert_eq!(first.between_study_variance_milli, 0);
        assert_eq!(first.random_effect_milli, first.pooled_effect_milli);
        first.validate().unwrap();
    }

    #[test]
    fn contradictory_sites_are_negative_and_not_hidden() {
        let studies = vec![
            study("s1", "site-a", 400, 20, 4),
            study("s2", "site-b", -400, 20, 4),
            study("s3", "site-c", 390, 20, 4),
        ];
        let output = analyze_replication_meta_analysis(&request(), &studies).unwrap();
        assert_eq!(output.disposition, MetaAnalysisDisposition::Heterogeneous);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("direction-contradiction")));
        assert!(output.i2_milli > 200);
        assert!(output.between_study_variance_milli > 0);
    }

    #[test]
    fn replicate_floor_is_unresolved_not_imputed() {
        let studies = vec![
            study("s1", "site-a", 300, 20, 4),
            study("s2", "site-b", 300, 20, 1),
            study("s3", "site-c", 300, 20, 1),
        ];
        let output = analyze_replication_meta_analysis(&request(), &studies).unwrap();
        assert_eq!(output.disposition, MetaAnalysisDisposition::Unresolved);
        assert_eq!(output.included_order, vec!["s1"]);
        assert_eq!(output.excluded_order, vec!["s2", "s3"]);
    }
}
