//! Robust, aggregate-only consensus for multi-site preclinical glioma benchmarks.
//!
//! Each institution keeps its benchmark traces and raw observations local and contributes only
//! a typed score, baseline, uncertainty, and replicate count.  The engine computes a
//! fixed-point inverse-uncertainty pool, a weighted median (to expose sensitivity to a single
//! site), heterogeneity, direction contradictions, and leave-one-site-out influence.  A pooled
//! score is never promoted when the consortium is underpowered, contradictory, heterogeneous, or
//! dominated by one site.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkConsensus1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_SCORE_MILLI: u64 = 1_000_000;
const WEIGHT_SCALE: u128 = 1_000_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkRequest {
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub minimum_sites: usize,
    pub minimum_replicates_per_site: u16,
    pub effect_threshold_milli: u64,
    pub max_i2_milli: u16,
    pub min_signal_to_noise_milli: u64,
    pub max_site_spread_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkSite {
    pub site_id: String,
    pub study_id: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub artifact: LocalArtifactRef,
    pub baseline_score_milli: u64,
    pub candidate_score_milli: u64,
    pub uncertainty_milli: u64,
    pub replicate_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkSiteDisposition {
    Included,
    Underpowered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkContribution {
    pub site_id: String,
    pub study_id: String,
    pub disposition: FederatedBenchmarkSiteDisposition,
    pub baseline_score_milli: u64,
    pub candidate_score_milli: u64,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub weight_milli: u64,
    pub leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkDisposition {
    Qualified,
    Heterogeneous,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkConsensus {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub site_order: Vec<String>,
    pub included_order: Vec<String>,
    pub excluded_order: Vec<String>,
    pub contributions: Vec<FederatedBenchmarkContribution>,
    pub fixed_effect_milli: i64,
    pub robust_median_effect_milli: i64,
    pub pooled_uncertainty_milli: u64,
    pub signal_to_noise_milli: u64,
    pub cochran_q_milli: u64,
    pub i2_milli: u16,
    pub site_spread_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedBenchmarkDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkError {
    #[error("federated benchmark request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated benchmark site is invalid: {0}")]
    InvalidSite(String),
    #[error("federated benchmark output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated benchmark digest failed: {0}")]
    Digest(String),
}

fn sign(value: i64) -> i8 {
    value.signum() as i8
}

fn effect(site: &FederatedBenchmarkSite) -> i64 {
    site.candidate_score_milli as i64 - site.baseline_score_milli as i64
}

fn weight_for(uncertainty_milli: u64) -> u128 {
    let variance = u128::from(uncertainty_milli).saturating_mul(u128::from(uncertainty_milli));
    WEIGHT_SCALE / variance.max(1)
}

fn fixed_effect(sites: &[&FederatedBenchmarkSite]) -> (i64, u128) {
    let total = sites
        .iter()
        .map(|site| weight_for(site.uncertainty_milli))
        .sum::<u128>();
    if total == 0 {
        return (0, 0);
    }
    let numerator = sites
        .iter()
        .map(|site| i128::from(effect(site)) * weight_for(site.uncertainty_milli) as i128)
        .sum::<i128>();
    ((numerator / total as i128) as i64, total)
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

fn weighted_median(sites: &[&FederatedBenchmarkSite]) -> i64 {
    let mut ordered = sites.to_vec();
    ordered.sort_by(|left, right| {
        effect(left)
            .cmp(&effect(right))
            .then_with(|| left.site_id.cmp(&right.site_id))
    });
    let total = ordered
        .iter()
        .map(|site| weight_for(site.uncertainty_milli))
        .sum::<u128>();
    let target = total.saturating_add(1) / 2;
    let mut cumulative = 0_u128;
    for site in ordered {
        cumulative = cumulative.saturating_add(weight_for(site.uncertainty_milli));
        if cumulative >= target {
            return effect(site);
        }
    }
    0
}

fn digest_input(output: &FederatedBenchmarkConsensus) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "capability_id": output.capability_id,
        "benchmark_world": output.benchmark_world,
        "metric_name": output.metric_name,
        "site_order": output.site_order,
        "included_order": output.included_order,
        "excluded_order": output.excluded_order,
        "contributions": output.contributions,
        "fixed_effect_milli": output.fixed_effect_milli,
        "robust_median_effect_milli": output.robust_median_effect_milli,
        "pooled_uncertainty_milli": output.pooled_uncertainty_milli,
        "signal_to_noise_milli": output.signal_to_noise_milli,
        "cochran_q_milli": output.cochran_q_milli,
        "i2_milli": output.i2_milli,
        "site_spread_milli": output.site_spread_milli,
        "max_leave_one_out_shift_milli": output.max_leave_one_out_shift_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl FederatedBenchmarkConsensus {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.benchmark_world.trim().is_empty()
            || self.metric_name.trim().is_empty()
            || self.i2_milli > 1_000
            || self.site_order.windows(2).any(|pair| pair[0] >= pair[1])
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
                .any(|pair| pair[0].site_id >= pair[1].site_id)
            || self.contributions.iter().any(|item| {
                item.site_id.trim().is_empty()
                    || item.study_id.trim().is_empty()
                    || item.uncertainty_milli == 0
                    || item.weight_milli == 0
                    || item.baseline_score_milli > MAX_SCORE_MILLI
                    || item.candidate_score_milli > MAX_SCORE_MILLI
                    || item.effect_milli.unsigned_abs() > MAX_SCORE_MILLI
                    || (item.disposition == FederatedBenchmarkSiteDisposition::Included
                        && item.leave_one_out_shift_milli > MAX_SCORE_MILLI)
            })
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedBenchmarkError::InvalidOutput(
                "identity, bounds, uniqueness, or canonical ordering is invalid".into(),
            ));
        }
        let sites = self.site_order.iter().cloned().collect::<BTreeSet<_>>();
        let included = self.included_order.iter().cloned().collect::<BTreeSet<_>>();
        let excluded = self.excluded_order.iter().cloned().collect::<BTreeSet<_>>();
        let contribution_sites = self
            .contributions
            .iter()
            .map(|item| item.site_id.clone())
            .collect::<BTreeSet<_>>();
        if included.intersection(&excluded).next().is_some()
            || included.union(&excluded).cloned().collect::<BTreeSet<_>>() != sites
            || contribution_sites != sites
            || self.contributions.iter().any(|item| {
                (item.disposition == FederatedBenchmarkSiteDisposition::Included)
                    != included.contains(&item.site_id)
            })
        {
            return Err(FederatedBenchmarkError::InvalidOutput(
                "included, excluded, and contribution site partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBenchmarkError::InvalidOutput(
                "digest is not bound to the federated benchmark consensus".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_federated_benchmark(
    request: &FederatedBenchmarkRequest,
    sites: &[FederatedBenchmarkSite],
) -> Result<FederatedBenchmarkConsensus, FederatedBenchmarkError> {
    if request.objective.trim().is_empty()
        || request.capability_id.trim().is_empty()
        || request.benchmark_world.trim().is_empty()
        || request.metric_name.trim().is_empty()
        || request.minimum_sites == 0
        || request.minimum_replicates_per_site == 0
        || request.effect_threshold_milli == 0
        || request.max_i2_milli > 1_000
        || request.min_signal_to_noise_milli == 0
        || sites.is_empty()
        || sites.len() > MAX_SITES
        || request.minimum_sites > sites.len()
    {
        return Err(FederatedBenchmarkError::InvalidRequest(
            "objective, benchmark identity, site/replicate floors, thresholds, and bounded sites are required".into(),
        ));
    }
    let mut site_ids = BTreeSet::new();
    let mut study_ids = BTreeSet::new();
    for site in sites {
        site.artifact
            .validate()
            .map_err(|error| FederatedBenchmarkError::InvalidSite(error.to_string()))?;
        if site.site_id.trim().is_empty()
            || site.study_id.trim().is_empty()
            || site.capability_id != request.capability_id
            || site.benchmark_world != request.benchmark_world
            || site.metric_name != request.metric_name
            || site.model_system != request.model_system
            || site.baseline_score_milli > MAX_SCORE_MILLI
            || site.candidate_score_milli > MAX_SCORE_MILLI
            || site.uncertainty_milli == 0
            || site.uncertainty_milli > MAX_SCORE_MILLI
            || site.replicate_count == 0
            || !site_ids.insert(site.site_id.clone())
            || !study_ids.insert(site.study_id.clone())
        {
            return Err(FederatedBenchmarkError::InvalidSite(
                "site/study identity, benchmark binding, model, score, uncertainty, replicate, or uniqueness is invalid".into(),
            ));
        }
    }
    let mut site_order = site_ids.iter().cloned().collect::<Vec<_>>();
    let mut included = sites
        .iter()
        .filter(|site| site.replicate_count >= request.minimum_replicates_per_site)
        .collect::<Vec<_>>();
    included.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let included_order = included
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let included_set = included_order.iter().cloned().collect::<BTreeSet<_>>();
    let excluded_order = site_order
        .iter()
        .filter(|site_id| !included_set.contains(*site_id))
        .cloned()
        .collect::<Vec<_>>();
    let (fixed_effect_milli, total_weight) = fixed_effect(&included);
    let pooled_uncertainty_milli = if total_weight == 0 {
        0
    } else {
        integer_sqrt(WEIGHT_SCALE.checked_div(total_weight).unwrap_or(0)).min(u128::from(u64::MAX))
            as u64
    };
    let robust_median_effect_milli = weighted_median(&included);
    let signal_to_noise_milli = if pooled_uncertainty_milli == 0 {
        0
    } else {
        fixed_effect_milli
            .unsigned_abs()
            .saturating_mul(1_000)
            .checked_div(pooled_uncertainty_milli)
            .unwrap_or(0)
    };
    let mut q_numerator = 0_u128;
    for site in &included {
        let delta = i128::from(effect(site)) - i128::from(fixed_effect_milli);
        q_numerator = q_numerator.saturating_add(
            weight_for(site.uncertainty_milli)
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
    let mut max_leave_one_out_shift_milli = 0_u64;
    let mut contributions = Vec::with_capacity(included.len());
    for site in &included {
        let without = included
            .iter()
            .copied()
            .filter(|candidate| candidate.site_id != site.site_id)
            .collect::<Vec<_>>();
        let (without_effect, _) = fixed_effect(&without);
        let shift = fixed_effect_milli
            .saturating_sub(without_effect)
            .unsigned_abs();
        max_leave_one_out_shift_milli = max_leave_one_out_shift_milli.max(shift);
        contributions.push(FederatedBenchmarkContribution {
            site_id: site.site_id.clone(),
            study_id: site.study_id.clone(),
            disposition: FederatedBenchmarkSiteDisposition::Included,
            baseline_score_milli: site.baseline_score_milli,
            candidate_score_milli: site.candidate_score_milli,
            effect_milli: effect(site),
            uncertainty_milli: site.uncertainty_milli,
            weight_milli: weight_for(site.uncertainty_milli).min(u128::from(u64::MAX)) as u64,
            leave_one_out_shift_milli: shift,
        });
    }
    for site in sites
        .iter()
        .filter(|site| !included_set.contains(&site.site_id))
    {
        contributions.push(FederatedBenchmarkContribution {
            site_id: site.site_id.clone(),
            study_id: site.study_id.clone(),
            disposition: FederatedBenchmarkSiteDisposition::Underpowered,
            baseline_score_milli: site.baseline_score_milli,
            candidate_score_milli: site.candidate_score_milli,
            effect_milli: effect(site),
            uncertainty_milli: site.uncertainty_milli,
            weight_milli: weight_for(site.uncertainty_milli).min(u128::from(u64::MAX)) as u64,
            leave_one_out_shift_milli: 0,
        });
    }
    contributions.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if sites.len() < request.minimum_sites || included.len() < request.minimum_sites {
        negative_evidence.insert("minimum-site-count-not-met".into());
    }
    if !excluded_order.is_empty() {
        uncertainty.insert("one-or-more-sites-below-replicate-floor".into());
    }
    let signs = included
        .iter()
        .map(|site| sign(effect(site)))
        .filter(|direction| *direction != 0)
        .collect::<BTreeSet<_>>();
    if signs.len() > 1 {
        negative_evidence.insert("cross-site-direction-contradiction".into());
    }
    if i2_milli > request.max_i2_milli {
        uncertainty.insert("benchmark-effect-heterogeneity-exceeds-i2-tolerance".into());
    }
    let (min_effect, max_effect) =
        included
            .iter()
            .map(|site| effect(site))
            .fold((None, None), |(min, max), value| {
                (
                    Some(min.map_or(value, |current: i64| current.min(value))),
                    Some(max.map_or(value, |current: i64| current.max(value))),
                )
            });
    let site_spread_milli = match (min_effect, max_effect) {
        (Some(min), Some(max)) => max.saturating_sub(min).unsigned_abs(),
        _ => 0,
    };
    if site_spread_milli > request.max_site_spread_milli {
        uncertainty.insert("cross-site-score-spread-exceeds-bound".into());
    }
    if max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli {
        uncertainty.insert("consensus-is-leave-one-site-sensitive".into());
    }
    if fixed_effect_milli.unsigned_abs() < request.effect_threshold_milli
        || robust_median_effect_milli.unsigned_abs() < request.effect_threshold_milli
        || signal_to_noise_milli < request.min_signal_to_noise_milli
    {
        negative_evidence.insert("pooled-or-robust-effect-does-not-clear-signal-threshold".into());
    }
    if fixed_effect_milli < 0 && robust_median_effect_milli < 0 {
        negative_evidence.insert("candidate-underperforms-baseline".into());
    }
    let disposition =
        if sites.len() < request.minimum_sites || included.len() < request.minimum_sites {
            FederatedBenchmarkDisposition::Unresolved
        } else if i2_milli > request.max_i2_milli || signs.len() > 1 {
            FederatedBenchmarkDisposition::Heterogeneous
        } else if !negative_evidence.is_empty() {
            FederatedBenchmarkDisposition::Negative
        } else if !uncertainty.is_empty() {
            FederatedBenchmarkDisposition::Unresolved
        } else {
            FederatedBenchmarkDisposition::Qualified
        };
    let mut output = FederatedBenchmarkConsensus {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        capability_id: request.capability_id.clone(),
        benchmark_world: request.benchmark_world.clone(),
        metric_name: request.metric_name.clone(),
        site_order: std::mem::take(&mut site_order),
        included_order,
        excluded_order,
        contributions,
        fixed_effect_milli,
        robust_median_effect_milli,
        pooled_uncertainty_milli,
        signal_to_noise_milli,
        cochran_q_milli,
        i2_milli,
        site_spread_milli,
        max_leave_one_out_shift_milli,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| FederatedBenchmarkError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }
    fn site(id: &str, score: u64, uncertainty: u64, replicates: u16) -> FederatedBenchmarkSite {
        FederatedBenchmarkSite {
            site_id: format!("site-{id}"),
            study_id: format!("study-{id}"),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-benchmark+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            baseline_score_milli: 500,
            candidate_score_milli: score,
            uncertainty_milli: uncertainty,
            replicate_count: replicates,
        }
    }
    fn request() -> FederatedBenchmarkRequest {
        FederatedBenchmarkRequest {
            objective: "compare invasion model improvements".into(),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 3,
            minimum_replicates_per_site: 3,
            effect_threshold_milli: 50,
            max_i2_milli: 250,
            min_signal_to_noise_milli: 500,
            max_site_spread_milli: 80,
            max_leave_one_out_shift_milli: 60,
        }
    }

    #[test]
    fn stable_site_consensus_is_qualified_and_replay_stable() {
        let sites = vec![
            site("a", 620, 20, 4),
            site("b", 625, 22, 4),
            site("c", 618, 21, 4),
        ];
        let first = analyze_federated_benchmark(&request(), &sites).unwrap();
        let second = analyze_federated_benchmark(&request(), &sites).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, FederatedBenchmarkDisposition::Qualified);
        assert_eq!(first.included_order.len(), 3);
        first.validate().unwrap();
    }

    #[test]
    fn contradictory_site_direction_is_heterogeneous_and_visible() {
        let sites = vec![
            site("a", 700, 20, 4),
            site("b", 300, 20, 4),
            site("c", 690, 20, 4),
        ];
        let output = analyze_federated_benchmark(&request(), &sites).unwrap();
        assert_eq!(
            output.disposition,
            FederatedBenchmarkDisposition::Heterogeneous
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("direction-contradiction")));
    }

    #[test]
    fn replicate_floor_is_unresolved_and_not_imputed() {
        let sites = vec![
            site("a", 620, 20, 4),
            site("b", 625, 22, 1),
            site("c", 618, 21, 1),
        ];
        let output = analyze_federated_benchmark(&request(), &sites).unwrap();
        assert_eq!(
            output.disposition,
            FederatedBenchmarkDisposition::Unresolved
        );
        assert_eq!(output.included_order, vec!["site-a"]);
        assert_eq!(output.excluded_order, vec!["site-b", "site-c"]);
    }
}
