//! Aggregate-only federated benchmark power and sufficiency analysis.
//!
//! Consensus is not the same as evidence sufficiency. This feature estimates the information
//! available from permitted site summaries, exposes heterogeneity and leave-one-site-out
//! influence, and computes a conservative fixed-point power proxy. It never requests raw data,
//! pools human data, or turns a benchmark into a clinical decision.

use super::consensus::FederatedBenchmarkSite;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkPower1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_SCORE_MILLI: u64 = 1_000_000;
const WEIGHT_SCALE: u128 = 1_000_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkPowerRequest {
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub minimum_sites: usize,
    pub minimum_replicates_per_site: u16,
    pub target_effect_milli: u64,
    pub minimum_signal_to_noise_milli: u64,
    pub minimum_power_milli: u16,
    pub max_heterogeneity_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkPowerSiteDisposition {
    Included,
    Underpowered,
    BindingMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkPowerContribution {
    pub site_id: String,
    pub study_id: String,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub replicate_count: u16,
    pub weight_milli: u64,
    pub leave_one_out_shift_milli: u64,
    pub disposition: FederatedBenchmarkPowerSiteDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkPowerDisposition {
    Qualified,
    Underpowered,
    Heterogeneous,
    InfluenceBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkPower {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub site_order: Vec<String>,
    pub included_order: Vec<String>,
    pub excluded_order: Vec<String>,
    pub contributions: Vec<FederatedBenchmarkPowerContribution>,
    pub pooled_effect_milli: i64,
    pub pooled_uncertainty_milli: u64,
    pub total_information_milli: u64,
    pub signal_to_noise_milli: u64,
    pub power_proxy_milli: u16,
    pub heterogeneity_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedBenchmarkPowerDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkPowerError {
    #[error("federated benchmark power request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated benchmark power site is invalid: {0}")]
    InvalidSite(String),
    #[error("federated benchmark power output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated benchmark power digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn effect(site: &FederatedBenchmarkSite) -> i64 {
    site.candidate_score_milli as i64 - site.baseline_score_milli as i64
}

fn weight(site: &FederatedBenchmarkSite) -> u128 {
    let variance = u128::from(site.uncertainty_milli)
        .saturating_mul(u128::from(site.uncertainty_milli))
        .max(1);
    WEIGHT_SCALE.saturating_mul(u128::from(site.replicate_count.max(1))) / variance
}

fn integer_sqrt(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }
    let mut low = 0_u128;
    let mut high = value.saturating_add(1);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

fn pooled_effect(sites: &[&FederatedBenchmarkSite]) -> (i64, u128) {
    let total_weight = sites.iter().map(|site| weight(site)).sum::<u128>();
    if total_weight == 0 {
        return (0, 0);
    }
    let numerator = sites
        .iter()
        .map(|site| i128::from(effect(site)) * weight(site) as i128)
        .sum::<i128>();
    ((numerator / total_weight as i128) as i64, total_weight)
}

fn digest_input(output: &FederatedBenchmarkPower) -> serde_json::Value {
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
        "pooled_effect_milli": output.pooled_effect_milli,
        "pooled_uncertainty_milli": output.pooled_uncertainty_milli,
        "total_information_milli": output.total_information_milli,
        "signal_to_noise_milli": output.signal_to_noise_milli,
        "power_proxy_milli": output.power_proxy_milli,
        "heterogeneity_milli": output.heterogeneity_milli,
        "max_leave_one_out_shift_milli": output.max_leave_one_out_shift_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl FederatedBenchmarkPower {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkPowerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.benchmark_world.trim().is_empty()
            || self.metric_name.trim().is_empty()
            || self.power_proxy_milli > 1_000
            || self.heterogeneity_milli > 1_000
            || !canonical(&self.site_order)
            || !canonical(&self.included_order)
            || !canonical(&self.excluded_order)
            || self
                .contributions
                .windows(2)
                .any(|pair| pair[0].site_id >= pair[1].site_id)
            || self
                .contributions
                .iter()
                .any(|item| item.site_id.trim().is_empty() || item.study_id.trim().is_empty())
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(FederatedBenchmarkPowerError::InvalidOutput(
                "identity, bounds, ordering, or contribution state is invalid".into(),
            ));
        }
        let site_ids = self.site_order.iter().cloned().collect::<BTreeSet<_>>();
        let contribution_ids = self
            .contributions
            .iter()
            .map(|item| item.site_id.clone())
            .collect::<BTreeSet<_>>();
        let included_ids = self.included_order.iter().cloned().collect::<BTreeSet<_>>();
        let excluded_ids = self.excluded_order.iter().cloned().collect::<BTreeSet<_>>();
        if site_ids.len() != self.site_order.len()
            || site_ids != contribution_ids
            || included_ids.intersection(&excluded_ids).next().is_some()
            || included_ids
                .union(&excluded_ids)
                .any(|id| !site_ids.contains(id))
        {
            return Err(FederatedBenchmarkPowerError::InvalidOutput(
                "site and disposition partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkPowerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBenchmarkPowerError::InvalidOutput(
                "digest is not bound to federated power output".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_federated_benchmark_power(
    request: &FederatedBenchmarkPowerRequest,
    sites: &[FederatedBenchmarkSite],
) -> Result<FederatedBenchmarkPower, FederatedBenchmarkPowerError> {
    if request.objective.trim().is_empty()
        || request.capability_id.trim().is_empty()
        || request.benchmark_world.trim().is_empty()
        || request.metric_name.trim().is_empty()
        || request.minimum_sites == 0
        || request.minimum_sites > MAX_SITES
        || request.minimum_replicates_per_site == 0
        || request.target_effect_milli == 0
        || request.minimum_power_milli > 1_000
        || request.max_heterogeneity_milli > 1_000
    {
        return Err(FederatedBenchmarkPowerError::InvalidRequest(
            "objective, benchmark binding, site/replicate floor, effect, power, and heterogeneity bounds are invalid"
                .into(),
        ));
    }
    if sites.is_empty() || sites.len() > MAX_SITES {
        return Err(FederatedBenchmarkPowerError::InvalidSite(
            "site count is outside the supported bound".into(),
        ));
    }
    let mut site_ids = BTreeSet::new();
    let mut eligible = Vec::new();
    let mut excluded = Vec::new();
    for site in sites {
        if !site_ids.insert(site.site_id.clone())
            || site.site_id.trim().is_empty()
            || site.study_id.trim().is_empty()
            || site.capability_id.trim().is_empty()
            || site.benchmark_world.trim().is_empty()
            || site.metric_name.trim().is_empty()
            || site.baseline_score_milli > MAX_SCORE_MILLI
            || site.candidate_score_milli > MAX_SCORE_MILLI
            || site.uncertainty_milli == 0
            || site.replicate_count == 0
            || site.artifact.contains_human_data
            || site.artifact.contains_direct_identifiers
        {
            return Err(FederatedBenchmarkPowerError::InvalidSite(
                "site identity, bounds, uncertainty, replicate, or privacy declaration is invalid"
                    .into(),
            ));
        }
        let binding_matches = site.capability_id == request.capability_id
            && site.benchmark_world == request.benchmark_world
            && site.metric_name == request.metric_name
            && site.model_system == request.model_system;
        if binding_matches && site.replicate_count >= request.minimum_replicates_per_site {
            eligible.push(site);
        } else {
            excluded.push(site);
        }
    }
    let pooled = pooled_effect(&eligible);
    let pooled_effect_milli = pooled.0;
    let total_information = pooled.1;
    let pooled_uncertainty_milli = if total_information == 0 {
        0
    } else {
        (1_000_000_u128 / integer_sqrt(total_information).max(1)) as u64
    };
    let signal_to_noise_milli = if pooled_uncertainty_milli == 0 {
        0
    } else {
        (pooled_effect_milli.unsigned_abs() as u128)
            .saturating_mul(1_000)
            .checked_div(u128::from(pooled_uncertainty_milli))
            .unwrap_or(0)
            .min(u128::from(u64::MAX)) as u64
    };
    let power_proxy_milli = ((u128::from(signal_to_noise_milli).saturating_mul(1_000))
        / u128::from(signal_to_noise_milli.saturating_add(1_960)))
    .min(1_000) as u16;
    let heterogeneity_milli = if eligible.is_empty() {
        1_000
    } else {
        let max_deviation = eligible
            .iter()
            .map(|site| effect(site).abs_diff(pooled_effect_milli))
            .max()
            .unwrap_or(0);
        (max_deviation
            .saturating_mul(1_000)
            .checked_div(
                pooled_effect_milli
                    .unsigned_abs()
                    .saturating_add(request.target_effect_milli)
                    .saturating_add(1),
            )
            .unwrap_or(1_000)
            .min(1_000)) as u16
    };
    let max_leave_one_out_shift_milli = eligible
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let without = eligible
                .iter()
                .enumerate()
                .filter_map(|(candidate_index, site)| (candidate_index != index).then_some(*site))
                .collect::<Vec<_>>();
            pooled_effect(&without).0.abs_diff(pooled_effect_milli)
        })
        .max()
        .unwrap_or(0);
    let mut contributions = sites
        .iter()
        .map(|site| {
            let is_included = eligible
                .iter()
                .any(|candidate| candidate.site_id == site.site_id);
            let shift = if is_included {
                let without = eligible
                    .iter()
                    .filter(|candidate| candidate.site_id != site.site_id)
                    .copied()
                    .collect::<Vec<_>>();
                pooled_effect(&without).0.abs_diff(pooled_effect_milli)
            } else {
                0
            };
            FederatedBenchmarkPowerContribution {
                site_id: site.site_id.clone(),
                study_id: site.study_id.clone(),
                effect_milli: effect(site),
                uncertainty_milli: site.uncertainty_milli,
                replicate_count: site.replicate_count,
                weight_milli: weight(site).min(u128::from(u64::MAX)) as u64,
                leave_one_out_shift_milli: shift,
                disposition: if is_included {
                    FederatedBenchmarkPowerSiteDisposition::Included
                } else if site.capability_id != request.capability_id
                    || site.benchmark_world != request.benchmark_world
                    || site.metric_name != request.metric_name
                    || site.model_system != request.model_system
                {
                    FederatedBenchmarkPowerSiteDisposition::BindingMismatch
                } else {
                    FederatedBenchmarkPowerSiteDisposition::Underpowered
                },
            }
        })
        .collect::<Vec<_>>();
    contributions.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let site_order = site_ids.iter().cloned().collect::<Vec<_>>();
    let included_order = eligible
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let excluded_order = excluded
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if included_order.len() < request.minimum_sites {
        negative_evidence.push("eligible site count is below the consortium floor".into());
    }
    if signal_to_noise_milli < request.minimum_signal_to_noise_milli
        || power_proxy_milli < request.minimum_power_milli
    {
        negative_evidence
            .push("pooled signal or conservative power proxy is below the release floor".into());
    }
    if heterogeneity_milli > request.max_heterogeneity_milli {
        negative_evidence.push("between-site heterogeneity exceeds the release ceiling".into());
    }
    if max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli {
        negative_evidence
            .push("one-site removal changes the pooled effect beyond the influence ceiling".into());
    }
    if excluded
        .iter()
        .any(|site| site.replicate_count < request.minimum_replicates_per_site)
    {
        uncertainty.push("one or more binding-matched sites are under-replicated".into());
    }
    if eligible
        .iter()
        .any(|site| site.uncertainty_milli > request.target_effect_milli)
    {
        uncertainty.push("at least one included site uncertainty exceeds the target effect".into());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if eligible.is_empty() {
        FederatedBenchmarkPowerDisposition::Unresolved
    } else if included_order.len() < request.minimum_sites
        || signal_to_noise_milli < request.minimum_signal_to_noise_milli
        || power_proxy_milli < request.minimum_power_milli
    {
        FederatedBenchmarkPowerDisposition::Underpowered
    } else if heterogeneity_milli > request.max_heterogeneity_milli {
        FederatedBenchmarkPowerDisposition::Heterogeneous
    } else if max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli {
        FederatedBenchmarkPowerDisposition::InfluenceBlocked
    } else {
        FederatedBenchmarkPowerDisposition::Qualified
    };
    let mut output = FederatedBenchmarkPower {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        capability_id: request.capability_id.clone(),
        benchmark_world: request.benchmark_world.clone(),
        metric_name: request.metric_name.clone(),
        site_order,
        included_order,
        excluded_order,
        contributions,
        pooled_effect_milli,
        pooled_uncertainty_milli,
        total_information_milli: total_information.min(u128::from(u64::MAX)) as u64,
        signal_to_noise_milli,
        power_proxy_milli,
        heterogeneity_milli,
        max_leave_one_out_shift_milli,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| FederatedBenchmarkPowerError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkPowerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn site(id: &str, effect: u64, uncertainty: u64) -> FederatedBenchmarkSite {
        FederatedBenchmarkSite {
            site_id: id.into(),
            study_id: format!("study-{id}"),
            capability_id: "segmentation".into(),
            benchmark_world: "world-1".into(),
            metric_name: "dice".into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
                content_type: "aggregate".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            baseline_score_milli: 500,
            candidate_score_milli: 500 + effect,
            uncertainty_milli: uncertainty,
            replicate_count: 5,
        }
    }

    fn request() -> FederatedBenchmarkPowerRequest {
        FederatedBenchmarkPowerRequest {
            objective: "benchmark glioma segmentation".into(),
            capability_id: "segmentation".into(),
            benchmark_world: "world-1".into(),
            metric_name: "dice".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 2,
            minimum_replicates_per_site: 3,
            target_effect_milli: 200,
            minimum_signal_to_noise_milli: 100,
            minimum_power_milli: 300,
            max_heterogeneity_milli: 500,
            max_leave_one_out_shift_milli: 200,
        }
    }

    #[test]
    fn qualified_sites_clear_power_and_influence_gates() {
        let output = analyze_federated_benchmark_power(
            &request(),
            &[site("site-a", 500, 20), site("site-b", 520, 20)],
        )
        .expect("power analysis");
        assert_eq!(
            output.disposition,
            FederatedBenchmarkPowerDisposition::Qualified
        );
        assert!(output.power_proxy_milli >= 300);
        output.validate().expect("digest validates");
    }

    #[test]
    fn heterogeneous_sites_remain_negative_evidence() {
        let output = analyze_federated_benchmark_power(
            &request(),
            &[site("site-a", 900, 20), site("site-b", 0, 20)],
        )
        .expect("power analysis");
        assert_eq!(
            output.disposition,
            FederatedBenchmarkPowerDisposition::Heterogeneous
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("heterogeneity")));
    }
}
