//! Small-consortium bias assessment for aggregate-only preclinical glioma benchmarks.
//!
//! A fixed-effect estimate from a handful of institutions can be pulled by one site even when
//! every site contributes only permitted summaries.  This feature uses a deterministic delete-one
//! jackknife over eligible site effects, reports the robust-median contrast, exposes heterogeneity,
//! and produces a site-count sensitivity surface.  The correction is an explicit finite-consortium
//! sensitivity model, not a claim that aggregate summaries can recover hidden site-level data.
//! Underpowered, heterogeneous, and high-bias results remain visible but cannot be promoted as a
//! qualified benchmark conclusion.

use super::consensus::{FederatedBenchmarkRequest, FederatedBenchmarkSite};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkBiasAssessment1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_SENSITIVITY_POINTS: usize = 64;
const WEIGHT_SCALE: u128 = 1_000_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkBiasCorrectionRequest {
    pub benchmark: FederatedBenchmarkRequest,
    pub target_site_count: usize,
    pub site_count_sensitivity: Vec<usize>,
    pub maximum_bias_bound_milli: u64,
    pub maximum_heterogeneity_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkBiasSensitivityPoint {
    pub assumed_site_count: usize,
    pub correction_milli: i64,
    pub corrected_effect_milli: i64,
    pub bias_bound_milli: u64,
    pub disposition: FederatedBenchmarkBiasSensitivityDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkBiasSensitivityDisposition {
    Stable,
    SiteCountSensitive,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkBiasDisposition {
    Corrected,
    Negative,
    Underpowered,
    Heterogeneous,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkBiasAssessment {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub site_order: Vec<String>,
    pub included_order: Vec<String>,
    pub excluded_order: Vec<String>,
    pub included_site_count: usize,
    pub target_site_count: usize,
    pub raw_fixed_effect_milli: i64,
    pub robust_median_effect_milli: i64,
    pub jackknife_bias_milli: i64,
    pub bias_adjusted_effect_milli: i64,
    pub bias_bound_milli: u64,
    pub heterogeneity_i2_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
    pub sensitivity: Vec<FederatedBenchmarkBiasSensitivityPoint>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedBenchmarkBiasDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkBiasError {
    #[error("federated benchmark bias request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated benchmark bias site is invalid: {0}")]
    InvalidSite(String),
    #[error("federated benchmark bias output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated benchmark bias digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn canonical_usize(values: &[usize]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn effect(site: &FederatedBenchmarkSite) -> i64 {
    site.candidate_score_milli as i64 - site.baseline_score_milli as i64
}

fn weight(uncertainty_milli: u64) -> u128 {
    let variance = u128::from(uncertainty_milli)
        .saturating_mul(u128::from(uncertainty_milli))
        .max(1);
    WEIGHT_SCALE / variance
}

fn pooled_effect(sites: &[&FederatedBenchmarkSite]) -> i64 {
    let total_weight = sites
        .iter()
        .map(|site| weight(site.uncertainty_milli))
        .sum::<u128>();
    if total_weight == 0 {
        return 0;
    }
    let numerator = sites
        .iter()
        .map(|site| i128::from(effect(site)) * weight(site.uncertainty_milli) as i128)
        .sum::<i128>();
    (numerator / total_weight as i128) as i64
}

fn weighted_median(sites: &[&FederatedBenchmarkSite]) -> i64 {
    let mut ordered = sites.to_vec();
    ordered.sort_by(|left, right| {
        effect(left)
            .cmp(&effect(right))
            .then_with(|| left.site_id.cmp(&right.site_id))
    });
    let total_weight = ordered
        .iter()
        .map(|site| weight(site.uncertainty_milli))
        .sum::<u128>();
    let target = total_weight.saturating_add(1) / 2;
    let mut cumulative = 0_u128;
    for site in ordered {
        cumulative = cumulative.saturating_add(weight(site.uncertainty_milli));
        if cumulative >= target {
            return effect(site);
        }
    }
    0
}

fn abs_difference(left: i64, right: i64) -> u64 {
    (i128::from(left) - i128::from(right)).unsigned_abs() as u64
}

fn mean_i64(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    (values.iter().map(|value| i128::from(*value)).sum::<i128>() / i128::from(values.len() as i64))
        as i64
}

fn weighted_heterogeneity_i2(sites: &[&FederatedBenchmarkSite]) -> u16 {
    if sites.len() < 2 {
        return 1_000;
    }
    let total_weight = sites
        .iter()
        .map(|site| weight(site.uncertainty_milli))
        .sum::<u128>();
    if total_weight == 0 {
        return 1_000;
    }
    let pooled = pooled_effect(sites);
    // Cochran's Q is evaluated in integer arithmetic. Saturation is intentional: a very
    // discordant aggregate must remain maximally heterogeneous rather than wrap into a small Q.
    let q_numerator = sites
        .iter()
        .map(|site| {
            let difference = i128::from(effect(site)) - i128::from(pooled);
            let squared = difference
                .unsigned_abs()
                .saturating_mul(difference.unsigned_abs());
            weight(site.uncertainty_milli).saturating_mul(squared)
        })
        .fold(0_u128, |sum, value| sum.saturating_add(value));
    let q = q_numerator.checked_div(total_weight).unwrap_or(u128::MAX);
    let degrees_of_freedom = (sites.len() - 1) as u128;
    if q <= degrees_of_freedom {
        0
    } else {
        q.saturating_sub(degrees_of_freedom)
            .saturating_mul(1_000)
            .checked_div(q.max(1))
            .unwrap_or(1_000)
            .min(1_000) as u16
    }
}

fn digest_input(output: &FederatedBenchmarkBiasAssessment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "capability_id": output.capability_id,
        "benchmark_world": output.benchmark_world,
        "metric_name": output.metric_name,
        "model_system": output.model_system,
        "site_order": output.site_order,
        "included_order": output.included_order,
        "excluded_order": output.excluded_order,
        "included_site_count": output.included_site_count,
        "target_site_count": output.target_site_count,
        "raw_fixed_effect_milli": output.raw_fixed_effect_milli,
        "robust_median_effect_milli": output.robust_median_effect_milli,
        "jackknife_bias_milli": output.jackknife_bias_milli,
        "bias_adjusted_effect_milli": output.bias_adjusted_effect_milli,
        "bias_bound_milli": output.bias_bound_milli,
        "heterogeneity_i2_milli": output.heterogeneity_i2_milli,
        "max_leave_one_out_shift_milli": output.max_leave_one_out_shift_milli,
        "sensitivity": output.sensitivity,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &FederatedBenchmarkBiasCorrectionRequest,
    sites: &[FederatedBenchmarkSite],
) -> Result<(), FederatedBenchmarkBiasError> {
    if request.target_site_count < 2
        || request.target_site_count > MAX_SITES
        || request.maximum_bias_bound_milli == 0
        || request.maximum_heterogeneity_milli > 1_000
        || request.site_count_sensitivity.is_empty()
        || request.site_count_sensitivity.len() > MAX_SENSITIVITY_POINTS
        || !canonical_usize(&request.site_count_sensitivity)
        || request
            .site_count_sensitivity
            .iter()
            .any(|count| *count < 2 || *count > MAX_SITES)
        || sites.is_empty()
        || sites.len() > MAX_SITES
    {
        return Err(FederatedBenchmarkBiasError::InvalidRequest(
            "bounded target/sensitivity site counts, positive bias bound, heterogeneity gate, and sites are required".into(),
        ));
    }
    if request.benchmark.minimum_sites < 2 {
        return Err(FederatedBenchmarkBiasError::InvalidRequest(
            "benchmark minimum site floor must be at least two".into(),
        ));
    }
    Ok(())
}

impl FederatedBenchmarkBiasAssessment {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkBiasError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.benchmark_world.trim().is_empty()
            || self.metric_name.trim().is_empty()
            || self.heterogeneity_i2_milli > 1_000
            || !canonical(&self.site_order)
            || !canonical(&self.included_order)
            || !canonical(&self.excluded_order)
            || self.included_site_count != self.included_order.len()
            || self
                .included_order
                .iter()
                .any(|site| self.excluded_order.contains(site))
            || self
                .sensitivity
                .windows(2)
                .any(|pair| pair[0].assumed_site_count >= pair[1].assumed_site_count)
            || self
                .sensitivity
                .iter()
                .any(|point| point.assumed_site_count < 2)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(FederatedBenchmarkBiasError::InvalidOutput(
                "identity, site partitions, sensitivity ordering, and evidence invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkBiasError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBenchmarkBiasError::InvalidOutput(
                "federated benchmark bias digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Estimate and transparently correct small-consortium bias from aggregate site summaries.
pub fn assess_federated_benchmark_small_consortium_bias(
    request: &FederatedBenchmarkBiasCorrectionRequest,
    sites: &[FederatedBenchmarkSite],
) -> Result<FederatedBenchmarkBiasAssessment, FederatedBenchmarkBiasError> {
    validate_request(request, sites)?;
    let mut site_order = sites
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    site_order.sort();
    let mut seen = BTreeSet::new();
    if site_order
        .iter()
        .any(|site_id| !seen.insert(site_id.clone()))
    {
        return Err(FederatedBenchmarkBiasError::InvalidSite(
            "site identities must be unique".into(),
        ));
    }
    for site in sites {
        site.artifact
            .validate()
            .map_err(|error| FederatedBenchmarkBiasError::InvalidSite(error.to_string()))?;
        if site.site_id.trim().is_empty()
            || site.study_id.trim().is_empty()
            || site.capability_id != request.benchmark.capability_id
            || site.benchmark_world != request.benchmark.benchmark_world
            || site.metric_name != request.benchmark.metric_name
            || site.model_system != request.benchmark.model_system
            || site.uncertainty_milli == 0
            || site.replicate_count == 0
        {
            return Err(FederatedBenchmarkBiasError::InvalidSite(
                "site benchmark identity, model, uncertainty, replicate, and uniqueness bindings are required".into(),
            ));
        }
    }
    let mut ordered_sites = sites.iter().collect::<Vec<_>>();
    ordered_sites.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let included = ordered_sites
        .iter()
        .copied()
        .filter(|site| site.replicate_count >= request.benchmark.minimum_replicates_per_site)
        .collect::<Vec<_>>();
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
    let raw_fixed_effect = pooled_effect(&included);
    let robust_median_effect = weighted_median(&included);
    let mut leave_one_out_effects = Vec::new();
    let mut max_leave_one_out_shift = 0_u64;
    for site in &included {
        let without = included
            .iter()
            .copied()
            .filter(|candidate| candidate.site_id != site.site_id)
            .collect::<Vec<_>>();
        if without.is_empty() {
            continue;
        }
        let leave_one_out = pooled_effect(&without);
        max_leave_one_out_shift =
            max_leave_one_out_shift.max(abs_difference(raw_fixed_effect, leave_one_out));
        leave_one_out_effects.push(leave_one_out);
    }
    let n = included.len();
    let jackknife_bias = if n < 2 {
        0
    } else {
        let mean_leave_one_out = mean_i64(&leave_one_out_effects);
        (i128::from(n.saturating_sub(1) as i64)
            * (i128::from(mean_leave_one_out) - i128::from(raw_fixed_effect))) as i64
    };
    let bias_adjusted_effect = raw_fixed_effect.saturating_sub(jackknife_bias);
    let base_bias_bound = abs_difference(jackknife_bias, 0)
        .saturating_add(abs_difference(raw_fixed_effect, robust_median_effect));
    let mut sensitivity = Vec::new();
    for assumed_site_count in &request.site_count_sensitivity {
        let correction = if n < 2 {
            0
        } else {
            (i128::from(jackknife_bias) * i128::from(n as i64)
                / i128::from((*assumed_site_count).max(1) as i64)) as i64
        };
        let corrected_effect = raw_fixed_effect.saturating_sub(correction);
        let bias_bound = abs_difference(correction, 0)
            .saturating_add(abs_difference(raw_fixed_effect, robust_median_effect));
        let disposition = if n < request.benchmark.minimum_sites
            || *assumed_site_count < request.benchmark.minimum_sites
        {
            FederatedBenchmarkBiasSensitivityDisposition::Unresolved
        } else if bias_bound > request.maximum_bias_bound_milli {
            FederatedBenchmarkBiasSensitivityDisposition::SiteCountSensitive
        } else {
            FederatedBenchmarkBiasSensitivityDisposition::Stable
        };
        sensitivity.push(FederatedBenchmarkBiasSensitivityPoint {
            assumed_site_count: *assumed_site_count,
            correction_milli: correction,
            corrected_effect_milli: corrected_effect,
            bias_bound_milli: bias_bound,
            disposition,
        });
    }
    let heterogeneity_i2_milli = weighted_heterogeneity_i2(&included);
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if included.len() < request.target_site_count {
        uncertainty.push("target-site-count-not-reached".into());
    }
    if included.len() < request.benchmark.minimum_sites {
        negative_evidence.push("underpowered-small-consortium".into());
    }
    if heterogeneity_i2_milli > request.maximum_heterogeneity_milli {
        negative_evidence.push("site-heterogeneity-exceeds-correction-gate".into());
    }
    if max_leave_one_out_shift > request.maximum_bias_bound_milli {
        uncertainty.push("single-site-influence-exceeds-bias-bound".into());
    }
    if sensitivity.iter().any(|point| {
        point.disposition == FederatedBenchmarkBiasSensitivityDisposition::SiteCountSensitive
    }) {
        uncertainty.push("site-count-sensitivity-is-material".into());
    }
    let disposition = if included.len() < request.benchmark.minimum_sites {
        FederatedBenchmarkBiasDisposition::Underpowered
    } else if heterogeneity_i2_milli > request.maximum_heterogeneity_milli {
        FederatedBenchmarkBiasDisposition::Heterogeneous
    } else if base_bias_bound > request.maximum_bias_bound_milli {
        FederatedBenchmarkBiasDisposition::Unresolved
    } else if bias_adjusted_effect.unsigned_abs() < request.benchmark.effect_threshold_milli {
        FederatedBenchmarkBiasDisposition::Negative
    } else {
        FederatedBenchmarkBiasDisposition::Corrected
    };
    negative_evidence.sort();
    uncertainty.sort();
    let mut output = FederatedBenchmarkBiasAssessment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.benchmark.objective.clone(),
        capability_id: request.benchmark.capability_id.clone(),
        benchmark_world: request.benchmark.benchmark_world.clone(),
        metric_name: request.benchmark.metric_name.clone(),
        model_system: request.benchmark.model_system,
        site_order,
        included_order,
        excluded_order,
        included_site_count: n,
        target_site_count: request.target_site_count,
        raw_fixed_effect_milli: raw_fixed_effect,
        robust_median_effect_milli: robust_median_effect,
        jackknife_bias_milli: jackknife_bias,
        bias_adjusted_effect_milli: bias_adjusted_effect,
        bias_bound_milli: base_bias_bound,
        heterogeneity_i2_milli,
        max_leave_one_out_shift_milli: max_leave_one_out_shift,
        sensitivity,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-small-consortium-bias"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkBiasError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn site(site_id: &str, effect_milli: i64, uncertainty_milli: u64) -> FederatedBenchmarkSite {
        let baseline = 500_u64;
        let candidate = (i64::from(baseline as i32) + effect_milli).max(0) as u64;
        FederatedBenchmarkSite {
            site_id: site_id.into(),
            study_id: format!("study-{site_id}"),
            capability_id: "glioma-invasion-v1".into(),
            benchmark_world: "world-1".into(),
            metric_name: "invasion_score".into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{site_id}"),
                content_hash: ContentHash::of_bytes(site_id.as_bytes()),
                content_type: "application/vnd.aurora.glioma.aggregate+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            baseline_score_milli: baseline,
            candidate_score_milli: candidate,
            uncertainty_milli,
            replicate_count: 4,
        }
    }

    fn request() -> FederatedBenchmarkBiasCorrectionRequest {
        FederatedBenchmarkBiasCorrectionRequest {
            benchmark: FederatedBenchmarkRequest {
                objective: "compare glioma invasion computation".into(),
                capability_id: "glioma-invasion-v1".into(),
                benchmark_world: "world-1".into(),
                metric_name: "invasion_score".into(),
                model_system: GliomaModelSystem::Organoid,
                minimum_sites: 3,
                minimum_replicates_per_site: 3,
                effect_threshold_milli: 10,
                max_i2_milli: 900,
                min_signal_to_noise_milli: 1,
                max_site_spread_milli: 1_000,
                max_leave_one_out_shift_milli: 1_000,
            },
            target_site_count: 5,
            site_count_sensitivity: vec![3, 5, 8],
            maximum_bias_bound_milli: 60,
            maximum_heterogeneity_milli: 900,
        }
    }

    #[test]
    fn jackknife_correction_is_permutation_stable_and_exposes_sensitivity() {
        let first = assess_federated_benchmark_small_consortium_bias(
            &request(),
            &[
                site("site-b", 40, 20),
                site("site-a", 20, 20),
                site("site-c", 25, 20),
            ],
        )
        .unwrap();
        let second = assess_federated_benchmark_small_consortium_bias(
            &request(),
            &[
                site("site-c", 25, 20),
                site("site-a", 20, 20),
                site("site-b", 40, 20),
            ],
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.included_site_count, 3);
        assert_eq!(first.sensitivity.len(), 3);
        assert!(first
            .uncertainty
            .iter()
            .any(|item| item == "target-site-count-not-reached"));
    }

    #[test]
    fn high_site_heterogeneity_blocks_correction() {
        let mut request = request();
        request.maximum_heterogeneity_milli = 100;
        let output = assess_federated_benchmark_small_consortium_bias(
            &request,
            &[
                site("site-a", -200, 10),
                site("site-b", 200, 10),
                site("site-c", 210, 10),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            FederatedBenchmarkBiasDisposition::Heterogeneous
        );
        assert!(output
            .negative_evidence
            .contains(&"site-heterogeneity-exceeds-correction-gate".into()));
    }

    #[test]
    fn precision_aware_i2_detects_nonzero_center_heterogeneity() {
        let output = assess_federated_benchmark_small_consortium_bias(
            &request(),
            &[
                site("site-a", 100, 10),
                site("site-b", 110, 10),
                site("site-c", 120, 10),
            ],
        )
        .unwrap();
        assert!(output.heterogeneity_i2_milli > 900);
        assert_eq!(
            output.disposition,
            FederatedBenchmarkBiasDisposition::Heterogeneous
        );
        assert!(output
            .negative_evidence
            .contains(&"site-heterogeneity-exceeds-correction-gate".into()));
        output.validate().unwrap();
    }

    #[test]
    fn underpowered_consortium_never_becomes_qualified() {
        let mut request = request();
        request.benchmark.minimum_sites = 4;
        let output = assess_federated_benchmark_small_consortium_bias(
            &request,
            &[
                site("site-a", 20, 20),
                site("site-b", 22, 20),
                site("site-c", 21, 20),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            FederatedBenchmarkBiasDisposition::Underpowered
        );
        assert!(output
            .negative_evidence
            .contains(&"underpowered-small-consortium".into()));
        assert!(output.sensitivity.iter().all(|point| {
            point.disposition == FederatedBenchmarkBiasSensitivityDisposition::Unresolved
                || point.assumed_site_count < request.benchmark.minimum_sites
        }));
    }

    #[test]
    fn corrected_null_effect_is_published_as_negative() {
        let mut request = request();
        request.benchmark.effect_threshold_milli = 25;
        let output = assess_federated_benchmark_small_consortium_bias(
            &request,
            &[
                site("site-a", 5, 20),
                site("site-b", 4, 20),
                site("site-c", 6, 20),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            FederatedBenchmarkBiasDisposition::Negative
        );
        assert!(output.bias_adjusted_effect_milli.unsigned_abs() < 25);
    }
}
