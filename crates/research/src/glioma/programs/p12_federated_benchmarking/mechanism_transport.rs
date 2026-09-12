//! Federated mechanistic transport analysis for preclinical glioma programs.
//!
//! Institutions contribute only aggregate, de-identified mechanism effects, quality, replicate
//! counts, and bounded population signatures. The engine estimates whether a mechanism survives
//! model-system and site variation, computes leave-one-site-out fragility, and preserves negative
//! or heterogeneous findings. Raw observations never cross the federation boundary.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedMechanismTransport1@1";
pub const MAX_SITES: usize = 4_096;
pub const MAX_SIGNATURE_DIMENSIONS: usize = 256;
pub const MAX_EFFECT_MAGNITUDE_MILLI: u64 = 1_000_000_000;
const WEIGHT_SCALE: u128 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismTransportRequest {
    pub objective: String,
    pub mechanism_id: String,
    pub target_model_system: GliomaModelSystem,
    pub target_signature: Vec<i64>,
    pub min_sites: usize,
    pub min_replicates_per_site: u16,
    pub min_quality_milli: u16,
    pub similarity_scale_milli: u32,
    pub effect_threshold_milli: u64,
    pub min_signal_to_noise_milli: u64,
    pub max_heterogeneity_milli: u64,
    pub max_site_spread_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub require_target_model: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismSite {
    pub site_id: String,
    pub study_id: String,
    pub mechanism_id: String,
    pub model_system: GliomaModelSystem,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub quality_milli: u16,
    pub replicate_count: u16,
    pub population_signature: Vec<i64>,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismContribution {
    pub site_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub distance_milli: u64,
    pub similarity_milli: u16,
    pub weight_milli: u64,
    pub effect_milli: i64,
    pub leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedModelCoverage {
    pub model_system: GliomaModelSystem,
    pub site_order: Vec<String>,
    pub included_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedMechanismTransportDisposition {
    Qualified,
    Heterogeneous,
    Negative,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismTransportAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub mechanism_id: String,
    pub target_model_system: GliomaModelSystem,
    pub site_order: Vec<String>,
    pub included_order: Vec<String>,
    pub excluded_order: Vec<String>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub model_coverage: Vec<FederatedModelCoverage>,
    pub contributions: Vec<FederatedMechanismContribution>,
    pub pooled_effect_milli: i64,
    pub robust_median_effect_milli: i64,
    pub pooled_uncertainty_milli: u64,
    pub signal_to_noise_milli: u64,
    pub heterogeneity_milli: u64,
    pub site_spread_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedMechanismTransportDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedMechanismTransportError {
    #[error("federated mechanism transport request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated mechanism site is invalid: {0}")]
    InvalidSite(String),
    #[error("federated mechanism transport output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated mechanism transport digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@>".contains(&byte))
}

fn distance(left: &[i64], right: &[i64]) -> u64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| a.saturating_sub(*b).unsigned_abs())
        .fold(0_u64, |sum, value| sum.saturating_add(value))
}

fn similarity(distance: u64, scale_milli: u32) -> u16 {
    let scaled = distance
        .saturating_mul(1_000)
        .checked_div(u64::from(scale_milli.max(1)))
        .unwrap_or(1_000);
    1_000_u16.saturating_sub(scaled.min(1_000) as u16)
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut low = 1_u128;
    let mut high = value.min(u128::from(u64::MAX));
    let mut answer = 1_u128;
    while low <= high {
        let middle = low + (high - low) / 2;
        if middle <= value / middle {
            answer = middle;
            low = middle.saturating_add(1);
        } else {
            high = middle.saturating_sub(1);
        }
    }
    answer
}

fn weighted_effect(
    sites: &[&FederatedMechanismSite],
    request: &FederatedMechanismTransportRequest,
) -> (i64, u128) {
    let mut total_weight = 0_u128;
    let mut numerator = 0_i128;
    for site in sites {
        let distance = distance(&site.population_signature, &request.target_signature);
        let similarity = u128::from(similarity(distance, request.similarity_scale_milli));
        let quality = u128::from(site.quality_milli);
        let replicates = u128::from(site.replicate_count.max(1));
        let uncertainty = u128::from(site.uncertainty_milli.max(1));
        let weight = similarity
            .saturating_mul(quality)
            .saturating_mul(replicates)
            .saturating_mul(WEIGHT_SCALE)
            .checked_div(uncertainty.saturating_mul(uncertainty).max(1))
            .unwrap_or(0);
        total_weight = total_weight.saturating_add(weight);
        numerator = numerator.saturating_add(i128::from(site.effect_milli) * weight as i128);
    }
    if total_weight == 0 {
        (0, 0)
    } else {
        (
            (numerator / total_weight as i128).clamp(i128::from(i64::MIN), i128::from(i64::MAX))
                as i64,
            total_weight,
        )
    }
}

fn median(mut values: Vec<i64>) -> i64 {
    if values.is_empty() {
        return 0;
    }
    values.sort();
    values[values.len() / 2]
}

fn digest_input(output: &FederatedMechanismTransportAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "mechanism_id": output.mechanism_id,
        "target_model_system": output.target_model_system,
        "site_order": output.site_order,
        "included_order": output.included_order,
        "excluded_order": output.excluded_order,
        "model_system_order": output.model_system_order,
        "model_coverage": output.model_coverage,
        "contributions": output.contributions,
        "pooled_effect_milli": output.pooled_effect_milli,
        "robust_median_effect_milli": output.robust_median_effect_milli,
        "pooled_uncertainty_milli": output.pooled_uncertainty_milli,
        "signal_to_noise_milli": output.signal_to_noise_milli,
        "heterogeneity_milli": output.heterogeneity_milli,
        "site_spread_milli": output.site_spread_milli,
        "max_leave_one_out_shift_milli": output.max_leave_one_out_shift_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &FederatedMechanismTransportRequest,
) -> Result<(), FederatedMechanismTransportError> {
    if request.objective.trim().is_empty()
        || !valid_identifier(&request.mechanism_id)
        || request.target_signature.is_empty()
        || request.target_signature.len() > MAX_SIGNATURE_DIMENSIONS
        || request.min_sites < 2
        || request.min_sites > MAX_SITES
        || request.min_replicates_per_site == 0
        || request.min_quality_milli > 1_000
        || request.similarity_scale_milli == 0
        || request.effect_threshold_milli > MAX_EFFECT_MAGNITUDE_MILLI
        || request.min_signal_to_noise_milli > MAX_EFFECT_MAGNITUDE_MILLI
        || request.max_heterogeneity_milli > MAX_EFFECT_MAGNITUDE_MILLI
        || request.max_site_spread_milli > MAX_EFFECT_MAGNITUDE_MILLI
        || request.max_leave_one_out_shift_milli > MAX_EFFECT_MAGNITUDE_MILLI
    {
        return Err(FederatedMechanismTransportError::InvalidRequest(
            "objective, mechanism, signature, site, quality, threshold, and bound constraints are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_site(
    site: &FederatedMechanismSite,
    request: &FederatedMechanismTransportRequest,
) -> Result<(), FederatedMechanismTransportError> {
    if !valid_identifier(&site.site_id)
        || !valid_identifier(&site.study_id)
        || site.mechanism_id != request.mechanism_id
        || site.population_signature.len() != request.target_signature.len()
        || site.quality_milli > 1_000
        || site.replicate_count == 0
        || site.uncertainty_milli == 0
        || site.effect_milli.unsigned_abs() > MAX_EFFECT_MAGNITUDE_MILLI
    {
        return Err(FederatedMechanismTransportError::InvalidSite(
            "site identity, mechanism binding, signature, quality, replicate, uncertainty, or effect bounds are invalid".into(),
        ));
    }
    site.artifact
        .validate()
        .map_err(|error| FederatedMechanismTransportError::InvalidSite(error.to_string()))
}

impl FederatedMechanismTransportAnalysis {
    pub fn validate(&self) -> Result<(), FederatedMechanismTransportError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !valid_identifier(&self.mechanism_id)
            || !canonical(&self.site_order)
            || !canonical(&self.included_order)
            || !canonical(&self.excluded_order)
            || !canonical(&self.model_system_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.contributions.len() != self.included_order.len()
            || self.model_coverage.len() != self.model_system_order.len()
        {
            return Err(FederatedMechanismTransportError::InvalidOutput(
                "identity, ordering, or contribution cardinality is invalid".into(),
            ));
        }
        let sites = self.site_order.iter().collect::<BTreeSet<_>>();
        let included = self.included_order.iter().collect::<BTreeSet<_>>();
        let excluded = self.excluded_order.iter().collect::<BTreeSet<_>>();
        if !included.is_subset(&sites)
            || !excluded.is_subset(&sites)
            || included.intersection(&excluded).next().is_some()
            || sites.len() != included.len() + excluded.len()
            || self
                .contributions
                .iter()
                .any(|item| !included.contains(&item.site_id))
            || self.model_coverage.iter().any(|item| {
                !self.model_system_order.contains(&item.model_system)
                    || !canonical(&item.site_order)
                    || !canonical(&item.included_order)
            })
        {
            return Err(FederatedMechanismTransportError::InvalidOutput(
                "site partitions, contributions, or model coverage do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedMechanismTransportError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedMechanismTransportError::InvalidOutput(
                "federated mechanism transport digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Analyze whether an aggregate mechanism effect transports across federated preclinical sites.
/// Only value-level summaries and local artifact references are accepted; this function never
/// requests, reconstructs, or exports raw observations.
pub fn analyze_federated_mechanism_transport(
    request: &FederatedMechanismTransportRequest,
    sites: &[FederatedMechanismSite],
) -> Result<FederatedMechanismTransportAnalysis, FederatedMechanismTransportError> {
    validate_request(request)?;
    if sites.is_empty() || sites.len() > MAX_SITES {
        return Err(FederatedMechanismTransportError::InvalidRequest(
            "site count is empty or exceeds the federation bound".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut ordered = sites.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    for site in &ordered {
        validate_site(site, request)?;
        if !seen.insert(site.site_id.clone()) {
            return Err(FederatedMechanismTransportError::InvalidSite(
                "site identifiers must be unique".into(),
            ));
        }
    }
    let mut included = Vec::new();
    let mut excluded = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for site in &ordered {
        let distance = distance(&site.population_signature, &request.target_signature);
        let similarity = similarity(distance, request.similarity_scale_milli);
        if site.replicate_count < request.min_replicates_per_site {
            excluded.insert(site.site_id.clone());
            uncertainty.insert(format!("underpowered-site:{}", site.site_id));
        } else if site.quality_milli < request.min_quality_milli {
            excluded.insert(site.site_id.clone());
            uncertainty.insert(format!("low-quality-site:{}", site.site_id));
        } else if similarity == 0 {
            excluded.insert(site.site_id.clone());
            uncertainty.insert(format!("distant-site:{}", site.site_id));
        } else {
            included.push(*site);
        }
    }
    let site_order = ordered
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let included_order = included
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let excluded_order = excluded.into_iter().collect::<Vec<_>>();
    let model_system_order = ordered
        .iter()
        .map(|site| site.model_system)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut model_coverage = Vec::new();
    for model_system in &model_system_order {
        let mut model_sites = ordered
            .iter()
            .filter(|site| site.model_system == *model_system)
            .map(|site| site.site_id.clone())
            .collect::<Vec<_>>();
        model_sites.sort();
        let mut model_included = included
            .iter()
            .filter(|site| site.model_system == *model_system)
            .map(|site| site.site_id.clone())
            .collect::<Vec<_>>();
        model_included.sort();
        model_coverage.push(FederatedModelCoverage {
            model_system: *model_system,
            site_order: model_sites,
            included_order: model_included,
        });
    }
    if included.is_empty() {
        uncertainty.insert("no-site-clears-federated-quality-gates".into());
    }
    let (pooled_effect, total_weight) = weighted_effect(&included, request);
    let robust_median_effect = median(included.iter().map(|site| site.effect_milli).collect());
    let pooled_uncertainty = if total_weight == 0 {
        0
    } else {
        (integer_sqrt(WEIGHT_SCALE.saturating_mul(WEIGHT_SCALE) / total_weight.max(1)) as u64)
            .max(1)
    };
    let signal_to_noise = pooled_effect
        .unsigned_abs()
        .saturating_mul(1_000)
        .checked_div(pooled_uncertainty.max(1))
        .unwrap_or(0);
    let mut effects = included
        .iter()
        .map(|site| site.effect_milli)
        .collect::<Vec<_>>();
    effects.sort();
    let site_spread = effects
        .first()
        .zip(effects.last())
        .map(|(low, high)| high.saturating_sub(*low).unsigned_abs())
        .unwrap_or(0);
    let heterogeneity = if included.is_empty() {
        0
    } else {
        included
            .iter()
            .map(|site| {
                site.effect_milli
                    .saturating_sub(pooled_effect)
                    .unsigned_abs()
            })
            .sum::<u64>()
            / included.len() as u64
    };
    let mut max_leave_one_out_shift = 0_u64;
    let mut contributions = Vec::new();
    for site in &included {
        let without = included
            .iter()
            .filter(|candidate| candidate.site_id != site.site_id)
            .copied()
            .collect::<Vec<_>>();
        let (without_effect, _) = weighted_effect(&without, request);
        let shift = without_effect.saturating_sub(pooled_effect).unsigned_abs();
        max_leave_one_out_shift = max_leave_one_out_shift.max(shift);
        let distance = distance(&site.population_signature, &request.target_signature);
        let similarity = similarity(distance, request.similarity_scale_milli);
        let weight = {
            let quality = u128::from(site.quality_milli);
            let reps = u128::from(site.replicate_count.max(1));
            let uncertainty = u128::from(site.uncertainty_milli.max(1));
            u128::from(similarity)
                .saturating_mul(quality)
                .saturating_mul(reps)
                .saturating_mul(WEIGHT_SCALE)
                .checked_div(uncertainty.saturating_mul(uncertainty).max(1))
                .unwrap_or(0) as u64
        };
        contributions.push(FederatedMechanismContribution {
            site_id: site.site_id.clone(),
            study_id: site.study_id.clone(),
            model_system: site.model_system,
            distance_milli: distance,
            similarity_milli: similarity,
            weight_milli: weight,
            effect_milli: site.effect_milli,
            leave_one_out_shift_milli: shift,
        });
    }
    contributions.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let target_coverage = model_coverage
        .iter()
        .find(|item| item.model_system == request.target_model_system)
        .map(|item| !item.included_order.is_empty())
        .unwrap_or(false);
    if request.require_target_model && !target_coverage {
        uncertainty.insert("target-model-system-has-no-included-site".into());
    }
    let sign_conflict = included.iter().any(|site| site.effect_milli > 0)
        && included.iter().any(|site| site.effect_milli < 0);
    if sign_conflict {
        uncertainty.insert("cross-site-effect-direction-conflict".into());
    }
    let mut negative_evidence = BTreeSet::new();
    for site in &included {
        if site.effect_milli <= 0 {
            negative_evidence.insert(format!("site-nonpositive-effect:{}", site.site_id));
        }
    }
    if pooled_effect.unsigned_abs() < request.effect_threshold_milli {
        negative_evidence.insert("pooled-effect-below-declared-threshold".into());
    }
    let disposition = if included.len() < request.min_sites
        || (request.require_target_model && !target_coverage)
    {
        FederatedMechanismTransportDisposition::Unresolved
    } else if sign_conflict
        || heterogeneity > request.max_heterogeneity_milli
        || site_spread > request.max_site_spread_milli
        || max_leave_one_out_shift > request.max_leave_one_out_shift_milli
    {
        FederatedMechanismTransportDisposition::Heterogeneous
    } else if pooled_effect.unsigned_abs() < request.effect_threshold_milli
        || signal_to_noise < request.min_signal_to_noise_milli
    {
        FederatedMechanismTransportDisposition::Negative
    } else if !excluded_order.is_empty() {
        FederatedMechanismTransportDisposition::Partial
    } else {
        FederatedMechanismTransportDisposition::Qualified
    };
    if !excluded_order.is_empty() {
        uncertainty.insert("one-or-more-sites-were-excluded".into());
    }
    if disposition == FederatedMechanismTransportDisposition::Heterogeneous {
        negative_evidence.insert("transportability-heterogeneity-exceeds-gate".into());
    }
    let mut output = FederatedMechanismTransportAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        mechanism_id: request.mechanism_id.clone(),
        target_model_system: request.target_model_system,
        site_order,
        included_order,
        excluded_order,
        model_system_order,
        model_coverage,
        contributions,
        pooled_effect_milli: pooled_effect,
        robust_median_effect_milli: robust_median_effect,
        pooled_uncertainty_milli: pooled_uncertainty,
        signal_to_noise_milli: signal_to_noise,
        heterogeneity_milli: heterogeneity,
        site_spread_milli: site_spread,
        max_leave_one_out_shift_milli: max_leave_one_out_shift,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-mechanism-transport"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedMechanismTransportError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> FederatedMechanismTransportRequest {
        FederatedMechanismTransportRequest {
            objective: "transport invasion mechanism across organoid and mouse sites".into(),
            mechanism_id: "invasion-mechanism".into(),
            target_model_system: GliomaModelSystem::Organoid,
            target_signature: vec![0, 0, 0],
            min_sites: 2,
            min_replicates_per_site: 2,
            min_quality_milli: 700,
            similarity_scale_milli: 1_000,
            effect_threshold_milli: 500,
            min_signal_to_noise_milli: 500,
            max_heterogeneity_milli: 250,
            max_site_spread_milli: 1_000,
            max_leave_one_out_shift_milli: 500,
            require_target_model: true,
        }
    }

    fn site(
        id: &str,
        model_system: GliomaModelSystem,
        effect_milli: i64,
        signature: Vec<i64>,
    ) -> FederatedMechanismSite {
        FederatedMechanismSite {
            site_id: id.into(),
            study_id: format!("study-{id}"),
            mechanism_id: "invasion-mechanism".into(),
            model_system,
            effect_milli,
            uncertainty_milli: 100,
            quality_milli: 900,
            replicate_count: 3,
            population_signature: signature,
            artifact: artifact(id),
        }
    }

    #[test]
    fn qualified_transport_requires_cross_site_and_target_model_support() {
        let output = analyze_federated_mechanism_transport(
            &request(),
            &[
                site(
                    "organoid-a",
                    GliomaModelSystem::Organoid,
                    900,
                    vec![0, 0, 0],
                ),
                site("mouse-b", GliomaModelSystem::MouseModel, 850, vec![1, 0, 0]),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            FederatedMechanismTransportDisposition::Qualified
        );
        assert_eq!(output.included_order.len(), 2);
        assert!(output
            .model_coverage
            .iter()
            .any(|item| item.model_system == GliomaModelSystem::Organoid));
        output.validate().unwrap();
    }

    #[test]
    fn direction_conflict_is_heterogeneous_not_averaged_away() {
        let output = analyze_federated_mechanism_transport(
            &request(),
            &[
                site(
                    "organoid-a",
                    GliomaModelSystem::Organoid,
                    900,
                    vec![0, 0, 0],
                ),
                site(
                    "mouse-b",
                    GliomaModelSystem::MouseModel,
                    -800,
                    vec![0, 0, 0],
                ),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            FederatedMechanismTransportDisposition::Heterogeneous
        );
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "cross-site-effect-direction-conflict"));
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item == "transportability-heterogeneity-exceeds-gate"));
    }

    #[test]
    fn underpowered_site_is_excluded_and_not_imputed() {
        let mut weak = site("weak", GliomaModelSystem::MouseModel, 900, vec![0, 0, 0]);
        weak.replicate_count = 1;
        let output = analyze_federated_mechanism_transport(
            &request(),
            &[
                site(
                    "organoid-a",
                    GliomaModelSystem::Organoid,
                    900,
                    vec![0, 0, 0],
                ),
                weak,
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            FederatedMechanismTransportDisposition::Unresolved
        );
        assert_eq!(output.excluded_order, vec!["weak"]);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "underpowered-site:weak"));
    }

    #[test]
    fn input_permutation_replays_identically_and_human_artifact_is_refused() {
        let sites = vec![
            site(
                "organoid-a",
                GliomaModelSystem::Organoid,
                900,
                vec![0, 0, 0],
            ),
            site("mouse-b", GliomaModelSystem::MouseModel, 850, vec![1, 0, 0]),
        ];
        let reversed = sites.iter().cloned().rev().collect::<Vec<_>>();
        assert_eq!(
            analyze_federated_mechanism_transport(&request(), &sites).unwrap(),
            analyze_federated_mechanism_transport(&request(), &reversed).unwrap()
        );
        let mut human = sites[0].clone();
        human.artifact.contains_human_data = true;
        assert!(matches!(
            analyze_federated_mechanism_transport(&request(), &[human, sites[1].clone()]),
            Err(FederatedMechanismTransportError::InvalidSite(_))
        ));
    }
}
