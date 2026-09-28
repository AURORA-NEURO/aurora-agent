//! Heterogeneity-adaptive, privacy-aware federated benchmark power planning.
//!
//! This planner extends ordinary pooled-power proxies with site heterogeneity, attrition,
//! modality coverage, and privacy-noise penalties.  It searches a bounded deterministic surface
//! of site-count and replicate-multiplier portfolios, reports the cost/power/claim-width tradeoff,
//! and recommends either a feasible consortium design or an honest narrower claim.  It consumes
//! aggregate site envelopes only and never requests raw observations.

use super::consensus::FederatedBenchmarkSite;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaHeterogeneityAdaptivePowerPlan1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_SURFACE_POINTS: usize = 512;
pub const MAX_REPLICATE_MULTIPLIER: u16 = 16;
const SCORE_SCALE: u64 = 1_000;
const SNR_Z_PROXY: u128 = 1_960;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedPowerSiteEnvelope {
    pub site: FederatedBenchmarkSite,
    pub attrition_milli: u16,
    pub modality_coverage_milli: u16,
    pub privacy_noise_milli: u16,
    pub cost_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityAdaptivePowerRequest {
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub minimum_sites: usize,
    pub minimum_replicates_per_site: u16,
    pub target_effect_milli: u64,
    pub minimum_power_milli: u16,
    pub maximum_heterogeneity_milli: u16,
    pub maximum_privacy_noise_milli: u16,
    pub maximum_budget_units: u64,
    pub maximum_replicate_multiplier: u16,
    pub maximum_surface_points: usize,
    pub sites: Vec<FederatedPowerSiteEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSensitivityPoint {
    pub site_count: usize,
    pub replicate_multiplier: u16,
    pub site_order: Vec<String>,
    pub power_proxy_milli: u16,
    pub heterogeneity_milli: u16,
    pub privacy_noise_milli: u16,
    pub information_milli: u64,
    pub claim_width_milli: u64,
    pub cost_units: u64,
    pub adequate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerRecommendation {
    QualifiedPortfolio,
    AddSites,
    IncreaseReplicates,
    NarrowClaim,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityAdaptivePowerPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub candidate_site_order: Vec<String>,
    pub excluded_site_order: Vec<String>,
    pub surface: Vec<PowerSensitivityPoint>,
    pub recommendation: PowerRecommendation,
    pub recommended_site_order: Vec<String>,
    pub recommended_replicate_multiplier: u16,
    pub recommended_power_milli: u16,
    pub recommended_claim_width_milli: u64,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HeterogeneityAdaptivePowerError {
    #[error("heterogeneity-adaptive power request is invalid: {0}")]
    InvalidRequest(String),
    #[error("heterogeneity-adaptive power output is invalid: {0}")]
    InvalidOutput(String),
    #[error("heterogeneity-adaptive power digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.iter().all(|value| safe_text(value)) && values.windows(2).all(|pair| pair[0] != pair[1])
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

fn effect(site: &FederatedBenchmarkSite) -> i64 {
    site.candidate_score_milli as i64 - site.baseline_score_milli as i64
}

fn median_effect(sites: &[&FederatedPowerSiteEnvelope]) -> i64 {
    let mut effects = sites
        .iter()
        .map(|site| effect(&site.site))
        .collect::<Vec<_>>();
    effects.sort_unstable();
    effects[effects.len() / 2]
}

fn site_information(site: &FederatedPowerSiteEnvelope, median: i64, target: u64) -> u128 {
    let heterogeneity_component = effect(&site.site).abs_diff(median);
    let variance = u128::from(site.site.uncertainty_milli)
        .saturating_mul(u128::from(site.site.uncertainty_milli))
        .saturating_add(
            u128::from(site.privacy_noise_milli)
                .saturating_mul(u128::from(site.privacy_noise_milli)),
        )
        .saturating_add(
            u128::from(
                heterogeneity_component
                    .saturating_mul(SCORE_SCALE)
                    .checked_div(target.max(1))
                    .unwrap_or(SCORE_SCALE)
                    .min(SCORE_SCALE),
            )
            .saturating_mul(u128::from(site.site.uncertainty_milli.max(1))),
        )
        .max(1);
    let retained = u128::from(SCORE_SCALE.saturating_sub(u64::from(site.attrition_milli)))
        .saturating_mul(u128::from(site.modality_coverage_milli));
    u128::from(site.site.replicate_count)
        .saturating_mul(retained)
        .saturating_mul(1_000_000)
        .checked_div(variance)
        .unwrap_or(0)
}

fn point_body(point: &PowerSensitivityPoint) -> serde_json::Value {
    serde_json::json!({
        "site_count": point.site_count,
        "replicate_multiplier": point.replicate_multiplier,
        "site_order": point.site_order,
        "power_proxy_milli": point.power_proxy_milli,
        "heterogeneity_milli": point.heterogeneity_milli,
        "privacy_noise_milli": point.privacy_noise_milli,
        "information_milli": point.information_milli,
        "claim_width_milli": point.claim_width_milli,
        "cost_units": point.cost_units,
        "adequate": point.adequate,
    })
}

fn output_body(plan: &HeterogeneityAdaptivePowerPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "capability_id": plan.capability_id,
        "benchmark_world": plan.benchmark_world,
        "metric_name": plan.metric_name,
        "candidate_site_order": plan.candidate_site_order,
        "excluded_site_order": plan.excluded_site_order,
        "surface": plan.surface,
        "recommendation": plan.recommendation,
        "recommended_site_order": plan.recommended_site_order,
        "recommended_replicate_multiplier": plan.recommended_replicate_multiplier,
        "recommended_power_milli": plan.recommended_power_milli,
        "recommended_claim_width_milli": plan.recommended_claim_width_milli,
        "negative_evidence_order": plan.negative_evidence_order,
        "uncertainty_order": plan.uncertainty_order,
    })
}

fn validate_request(
    request: &HeterogeneityAdaptivePowerRequest,
) -> Result<(), HeterogeneityAdaptivePowerError> {
    if !safe_text(&request.objective)
        || !safe_text(&request.capability_id)
        || !safe_text(&request.benchmark_world)
        || !safe_text(&request.metric_name)
        || request.minimum_sites == 0
        || request.minimum_sites > MAX_SITES
        || request.minimum_replicates_per_site == 0
        || request.target_effect_milli == 0
        || request.minimum_power_milli > 1_000
        || request.maximum_heterogeneity_milli > 1_000
        || request.maximum_privacy_noise_milli > 1_000
        || request.maximum_budget_units == 0
        || request.maximum_replicate_multiplier == 0
        || request.maximum_replicate_multiplier > MAX_REPLICATE_MULTIPLIER
        || request.maximum_surface_points == 0
        || request.maximum_surface_points > MAX_SURFACE_POINTS
        || request.sites.is_empty()
        || request.sites.len() > MAX_SITES
    {
        return Err(HeterogeneityAdaptivePowerError::InvalidRequest(
            "binding, site/replicate floors, effect/power/heterogeneity/privacy bounds, budget, and bounded sensitivity surface are required".into(),
        ));
    }
    let mut site_ids = std::collections::BTreeSet::new();
    for envelope in &request.sites {
        let site = &envelope.site;
        if !safe_text(&site.site_id)
            || !site_ids.insert(site.site_id.clone())
            || !safe_text(&site.study_id)
            || !safe_text(&site.capability_id)
            || !safe_text(&site.benchmark_world)
            || !safe_text(&site.metric_name)
            || site.baseline_score_milli > 1_000_000
            || site.candidate_score_milli > 1_000_000
            || site.uncertainty_milli == 0
            || site.replicate_count == 0
            || site.artifact.contains_human_data
            || site.artifact.contains_direct_identifiers
            || envelope.attrition_milli > 1_000
            || envelope.modality_coverage_milli == 0
            || envelope.modality_coverage_milli > 1_000
            || envelope.privacy_noise_milli > 1_000
            || envelope.cost_units == 0
        {
            return Err(HeterogeneityAdaptivePowerError::InvalidRequest(format!(
                "site {} has invalid aggregate envelope, privacy, or cost bounds",
                site.site_id
            )));
        }
    }
    Ok(())
}

impl HeterogeneityAdaptivePowerPlan {
    pub fn validate(&self) -> Result<(), HeterogeneityAdaptivePowerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !safe_text(&self.capability_id)
            || !safe_text(&self.benchmark_world)
            || !safe_text(&self.metric_name)
            || !canonical(&self.candidate_site_order)
            || !canonical(&self.excluded_site_order)
            || !canonical(&self.recommended_site_order)
            || self.surface.is_empty()
            || self.surface.len() > MAX_SURFACE_POINTS
            || self.recommended_replicate_multiplier > MAX_REPLICATE_MULTIPLIER
            || self.recommended_power_milli > 1_000
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.surface.iter().any(|point| {
                point.site_count == 0
                    || point.replicate_multiplier == 0
                    || point.replicate_multiplier > MAX_REPLICATE_MULTIPLIER
                    || point.power_proxy_milli > 1_000
                    || point.heterogeneity_milli > 1_000
                    || point.privacy_noise_milli > 1_000
                    || !canonical(&point.site_order)
                    || ContentHash::of_value(&point_body(point)).is_err()
            })
        {
            return Err(HeterogeneityAdaptivePowerError::InvalidOutput(
                "power-plan identity, surface bounds, canonical partitions, or sensitivity point invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| HeterogeneityAdaptivePowerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(HeterogeneityAdaptivePowerError::InvalidOutput(
                "power-plan digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

pub fn plan_glioma_heterogeneity_adaptive_benchmark_power(
    request: &HeterogeneityAdaptivePowerRequest,
) -> Result<HeterogeneityAdaptivePowerPlan, HeterogeneityAdaptivePowerError> {
    validate_request(request)?;
    let mut candidate = request
        .sites
        .iter()
        .filter(|envelope| {
            let site = &envelope.site;
            site.capability_id == request.capability_id
                && site.benchmark_world == request.benchmark_world
                && site.metric_name == request.metric_name
                && site.model_system == request.model_system
                && site.replicate_count >= request.minimum_replicates_per_site
        })
        .collect::<Vec<_>>();
    let mut excluded = request
        .sites
        .iter()
        .filter(|envelope| {
            !candidate
                .iter()
                .any(|item| item.site.site_id == envelope.site.site_id)
        })
        .map(|envelope| envelope.site.site_id.clone())
        .collect::<Vec<_>>();
    candidate.sort_by(|left, right| left.site.site_id.cmp(&right.site.site_id));
    excluded.sort();
    let median = if candidate.is_empty() {
        0
    } else {
        median_effect(&candidate)
    };
    candidate.sort_by(|left, right| {
        let left_info = site_information(left, median, request.target_effect_milli);
        let right_info = site_information(right, median, request.target_effect_milli);
        right_info
            .saturating_mul(u128::from(right.cost_units.max(1)))
            .cmp(&left_info.saturating_mul(u128::from(left.cost_units.max(1))))
            .then_with(|| left.site.site_id.cmp(&right.site.site_id))
    });
    let candidate_site_order = candidate
        .iter()
        .map(|site| site.site.site_id.clone())
        .collect::<Vec<_>>();
    let mut surface = Vec::new();
    let mut qualified: Option<PowerSensitivityPoint> = None;
    for site_count in request.minimum_sites..=candidate.len() {
        let selected = &candidate[..site_count];
        let heterogeneity = selected
            .iter()
            .map(|site| {
                (effect(&site.site)
                    .abs_diff(median)
                    .saturating_mul(SCORE_SCALE)
                    .checked_div(request.target_effect_milli.max(1))
                    .unwrap_or(SCORE_SCALE)
                    .min(SCORE_SCALE)) as u16
            })
            .max()
            .unwrap_or(1_000);
        let privacy_noise = selected
            .iter()
            .map(|site| site.privacy_noise_milli)
            .max()
            .unwrap_or(1_000);
        let base_information = selected
            .iter()
            .map(|site| site_information(site, median, request.target_effect_milli))
            .sum::<u128>();
        let base_cost = selected.iter().map(|site| site.cost_units).sum::<u64>();
        for multiplier in 1..=request.maximum_replicate_multiplier {
            if surface.len() >= request.maximum_surface_points {
                break;
            }
            let information = base_information.saturating_mul(u128::from(multiplier));
            let snr = integer_sqrt(information)
                .saturating_mul(u128::from(request.target_effect_milli))
                .checked_div(1_000)
                .unwrap_or(0);
            let power =
                (snr.saturating_mul(1_000) / snr.saturating_add(SNR_Z_PROXY)).min(1_000) as u16;
            let claim_width = if snr == 0 {
                u64::MAX
            } else {
                (u128::from(request.target_effect_milli)
                    .saturating_mul(1_000)
                    .checked_div(snr)
                    .unwrap_or(u128::from(u64::MAX)))
                .min(u128::from(u64::MAX)) as u64
            };
            let cost = base_cost.saturating_mul(u64::from(multiplier));
            let adequate = site_count >= request.minimum_sites
                && power >= request.minimum_power_milli
                && heterogeneity <= request.maximum_heterogeneity_milli
                && privacy_noise <= request.maximum_privacy_noise_milli
                && cost <= request.maximum_budget_units;
            let point = PowerSensitivityPoint {
                site_count,
                replicate_multiplier: multiplier,
                site_order: selected
                    .iter()
                    .map(|site| site.site.site_id.clone())
                    .collect(),
                power_proxy_milli: power,
                heterogeneity_milli: heterogeneity,
                privacy_noise_milli: privacy_noise,
                information_milli: information.min(u128::from(u64::MAX)) as u64,
                claim_width_milli: claim_width,
                cost_units: cost,
                adequate,
            };
            if adequate
                && qualified.as_ref().is_none_or(|best| {
                    (point.cost_units, point.claim_width_milli)
                        < (best.cost_units, best.claim_width_milli)
                })
            {
                qualified = Some(point.clone());
            }
            surface.push(point);
        }
        if surface.len() >= request.maximum_surface_points {
            break;
        }
    }
    if surface.is_empty() {
        surface.push(PowerSensitivityPoint {
            site_count: candidate.len(),
            replicate_multiplier: 1,
            site_order: candidate
                .iter()
                .map(|site| site.site.site_id.clone())
                .collect(),
            power_proxy_milli: 0,
            heterogeneity_milli: 1_000,
            privacy_noise_milli: candidate
                .iter()
                .map(|site| site.privacy_noise_milli)
                .max()
                .unwrap_or(1_000),
            information_milli: 0,
            claim_width_milli: u64::MAX,
            cost_units: candidate.iter().map(|site| site.cost_units).sum(),
            adequate: false,
        });
    }
    let recommendation = if let Some(_) = qualified {
        PowerRecommendation::QualifiedPortfolio
    } else if candidate.len() < request.minimum_sites {
        PowerRecommendation::AddSites
    } else if surface
        .iter()
        .any(|point| point.power_proxy_milli >= request.minimum_power_milli)
    {
        PowerRecommendation::NarrowClaim
    } else if request.maximum_replicate_multiplier < MAX_REPLICATE_MULTIPLIER {
        PowerRecommendation::IncreaseReplicates
    } else {
        PowerRecommendation::Blocked
    };
    let chosen = qualified.unwrap_or_else(|| {
        surface
            .iter()
            .max_by_key(|point| (point.power_proxy_milli, point.site_count))
            .cloned()
            .unwrap_or(PowerSensitivityPoint {
                site_count: 0,
                replicate_multiplier: 0,
                site_order: Vec::new(),
                power_proxy_milli: 0,
                heterogeneity_milli: 1_000,
                privacy_noise_milli: 1_000,
                information_milli: 0,
                claim_width_milli: u64::MAX,
                cost_units: 0,
                adequate: false,
            })
    });
    let mut negative_evidence_order = Vec::new();
    let mut uncertainty_order = Vec::new();
    if candidate.len() < request.minimum_sites {
        negative_evidence_order
            .push("binding-matched site count is below the consortium floor".into());
    }
    if chosen.power_proxy_milli < request.minimum_power_milli {
        negative_evidence_order.push("all bounded portfolios remain underpowered".into());
    }
    if chosen.heterogeneity_milli > request.maximum_heterogeneity_milli {
        negative_evidence_order
            .push("site heterogeneity exceeds the declared claim ceiling".into());
    }
    if chosen.privacy_noise_milli > request.maximum_privacy_noise_milli {
        negative_evidence_order.push("privacy noise dominates the declared power envelope".into());
    }
    if !excluded.is_empty() {
        uncertainty_order.push("binding-mismatched or under-replicated sites were excluded without requesting raw data".into());
    }
    negative_evidence_order.sort();
    negative_evidence_order.dedup();
    uncertainty_order.sort();
    uncertainty_order.dedup();
    let mut plan = HeterogeneityAdaptivePowerPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        capability_id: request.capability_id.clone(),
        benchmark_world: request.benchmark_world.clone(),
        metric_name: request.metric_name.clone(),
        candidate_site_order,
        excluded_site_order: excluded,
        surface,
        recommendation,
        recommended_site_order: chosen.site_order,
        recommended_replicate_multiplier: chosen.replicate_multiplier,
        recommended_power_milli: chosen.power_proxy_milli,
        recommended_claim_width_milli: chosen.claim_width_milli,
        negative_evidence_order,
        uncertainty_order,
        digest: ContentHash::of_bytes(b"unsealed-glioma-heterogeneity-adaptive-power"),
    };
    plan.digest = ContentHash::of_value(&output_body(&plan))
        .map_err(|error| HeterogeneityAdaptivePowerError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn site(id: &str, effect_milli: u64, uncertainty_milli: u64) -> FederatedPowerSiteEnvelope {
        FederatedPowerSiteEnvelope {
            site: FederatedBenchmarkSite {
                site_id: id.into(),
                study_id: format!("study-{id}"),
                capability_id: "segmentation".into(),
                benchmark_world: "world-1".into(),
                metric_name: "dice".into(),
                model_system: GliomaModelSystem::Organoid,
                artifact: LocalArtifactRef {
                    artifact_id: format!("artifact-{id}"),
                    content_hash: ContentHash::of_bytes(id.as_bytes()),
                    content_type: "aggregate".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
                baseline_score_milli: 500,
                candidate_score_milli: 500 + effect_milli,
                uncertainty_milli,
                replicate_count: 5,
            },
            attrition_milli: 100,
            modality_coverage_milli: 900,
            privacy_noise_milli: 50,
            cost_units: 10,
        }
    }

    fn request(sites: Vec<FederatedPowerSiteEnvelope>) -> HeterogeneityAdaptivePowerRequest {
        HeterogeneityAdaptivePowerRequest {
            objective: "power a glioma segmentation benchmark".into(),
            capability_id: "segmentation".into(),
            benchmark_world: "world-1".into(),
            metric_name: "dice".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 2,
            minimum_replicates_per_site: 3,
            target_effect_milli: 200,
            minimum_power_milli: 300,
            maximum_heterogeneity_milli: 500,
            maximum_privacy_noise_milli: 200,
            maximum_budget_units: 200,
            maximum_replicate_multiplier: 4,
            maximum_surface_points: 32,
            sites,
        }
    }

    #[test]
    fn low_heterogeneity_portfolio_is_qualified() {
        let output = plan_glioma_heterogeneity_adaptive_benchmark_power(&request(vec![
            site("site-a", 500, 20),
            site("site-b", 510, 20),
            site("site-c", 490, 20),
        ]))
        .unwrap();
        assert_eq!(
            output.recommendation,
            PowerRecommendation::QualifiedPortfolio
        );
        assert!(output.recommended_power_milli >= 300);
        assert!(output.validate().is_ok());
    }

    #[test]
    fn heterogeneity_and_privacy_noise_remain_negative_evidence() {
        let mut noisy = site("site-b", 900, 20);
        noisy.privacy_noise_milli = 800;
        let output = plan_glioma_heterogeneity_adaptive_benchmark_power(&request(vec![
            site("site-a", 0, 20),
            noisy,
        ]))
        .unwrap();
        assert!(output
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("heterogeneity") || item.contains("privacy")));
    }

    #[test]
    fn binding_mismatch_is_excluded_without_raw_data_request() {
        let mut mismatch = site("site-b", 500, 20);
        mismatch.site.metric_name = "wrong-metric".into();
        let output = plan_glioma_heterogeneity_adaptive_benchmark_power(&request(vec![
            site("site-a", 500, 20),
            mismatch,
        ]))
        .unwrap();
        assert!(output.excluded_site_order.contains(&"site-b".to_string()));
        assert!(output
            .uncertainty_order
            .iter()
            .any(|item| item.contains("excluded")));
    }

    #[test]
    fn deterministic_surface_repeats_exactly() {
        let input = request(vec![site("site-a", 500, 20), site("site-b", 510, 20)]);
        let left = plan_glioma_heterogeneity_adaptive_benchmark_power(&input).unwrap();
        let right = plan_glioma_heterogeneity_adaptive_benchmark_power(&input).unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn insufficient_sites_are_explicitly_underpowered() {
        let mut input = request(vec![site("site-a", 100, 100)]);
        input.minimum_sites = 2;
        let output = plan_glioma_heterogeneity_adaptive_benchmark_power(&input).unwrap();
        assert_eq!(output.recommendation, PowerRecommendation::AddSites);
        assert!(output
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("site count")));
    }
}
