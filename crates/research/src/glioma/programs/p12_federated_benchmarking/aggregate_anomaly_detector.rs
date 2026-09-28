//! Aggregate-only anomaly detection for federated preclinical glioma benchmarks.
//!
//! Institutions export bounded summaries, not observations. The detector uses robust median/MAD
//! diagnostics, protocol and range checks, temporal comparisons, and privacy suppression signals.
//! It never imputes a site cause or silently drops a contribution: every anomaly becomes an
//! explainable local-review request and remains visible to downstream benchmark governance.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedAggregateAnomalyAssessment1@1";
pub const MAX_SITES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAggregateObservation {
    pub site_id: String,
    pub study_id: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub aggregate_value_milli: i64,
    pub uncertainty_milli: u64,
    pub replicate_count: u16,
    pub historical_value_milli: Option<i64>,
    pub protocol_version: String,
    pub privacy_suppressed: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub source_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAggregateAnomalyRequest {
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub protocol_version: String,
    pub model_system: GliomaModelSystem,
    pub minimum_sites: usize,
    pub minimum_replicates_per_site: u16,
    pub minimum_value_milli: i64,
    pub maximum_value_milli: i64,
    pub maximum_uncertainty_milli: u64,
    pub robust_outlier_threshold_milli: u64,
    pub maximum_temporal_drift_milli: u64,
    pub maximum_suppressed_fraction_milli: u16,
    pub observations: Vec<FederatedAggregateObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateAnomalyKind {
    ImpossibleValue,
    RobustOutlier,
    TemporalDrift,
    ProtocolMismatch,
    PrivacySuppressed,
    HighUncertainty,
    UnderReplicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateAnomalyDisposition {
    Clean,
    Review,
    Insufficient,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateAnomalyRecord {
    pub site_id: String,
    pub deviation_from_median_milli: u64,
    pub temporal_drift_milli: u64,
    pub kinds: Vec<AggregateAnomalyKind>,
    pub review_required: bool,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAggregateAnomalyAssessment {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub candidate_site_order: Vec<String>,
    pub excluded_site_order: Vec<String>,
    pub anomaly_order: Vec<String>,
    pub review_order: Vec<String>,
    pub clean_order: Vec<String>,
    pub records: Vec<AggregateAnomalyRecord>,
    pub robust_median_milli: i64,
    pub median_absolute_deviation_milli: u64,
    pub robust_threshold_milli: u64,
    pub suppressed_fraction_milli: u16,
    pub disposition: AggregateAnomalyDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedAggregateAnomalyError {
    #[error("aggregate anomaly request is invalid: {0}")]
    InvalidRequest(String),
    #[error("aggregate anomaly output is invalid: {0}")]
    InvalidOutput(String),
    #[error("aggregate anomaly digest failed: {0}")]
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

fn median(mut values: Vec<i64>) -> i64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn output_body(assessment: &FederatedAggregateAnomalyAssessment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": assessment.feature_id,
        "output_schema": assessment.output_schema,
        "objective": assessment.objective,
        "capability_id": assessment.capability_id,
        "benchmark_world": assessment.benchmark_world,
        "metric_name": assessment.metric_name,
        "candidate_site_order": assessment.candidate_site_order,
        "excluded_site_order": assessment.excluded_site_order,
        "anomaly_order": assessment.anomaly_order,
        "review_order": assessment.review_order,
        "clean_order": assessment.clean_order,
        "records": assessment.records,
        "robust_median_milli": assessment.robust_median_milli,
        "median_absolute_deviation_milli": assessment.median_absolute_deviation_milli,
        "robust_threshold_milli": assessment.robust_threshold_milli,
        "suppressed_fraction_milli": assessment.suppressed_fraction_milli,
        "disposition": assessment.disposition,
        "negative_evidence": assessment.negative_evidence,
        "uncertainty": assessment.uncertainty,
    })
}

impl FederatedAggregateAnomalyAssessment {
    pub fn validate(&self) -> Result<(), FederatedAggregateAnomalyError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !safe_text(&self.capability_id)
            || !safe_text(&self.benchmark_world)
            || !safe_text(&self.metric_name)
            || !canonical(&self.candidate_site_order)
            || !canonical(&self.excluded_site_order)
            || !canonical(&self.anomaly_order)
            || !canonical(&self.review_order)
            || !canonical(&self.clean_order)
            || self.records.iter().any(|record| {
                !safe_text(&record.site_id)
                    || (record.review_required && record.kinds.is_empty())
                    || !safe_text(&record.explanation)
            })
            || self.suppressed_fraction_milli > 1_000
        {
            return Err(FederatedAggregateAnomalyError::InvalidOutput(
                "anomaly identity, site partitions, records, or bounded suppression invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| FederatedAggregateAnomalyError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedAggregateAnomalyError::InvalidOutput(
                "anomaly assessment digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &FederatedAggregateAnomalyRequest,
) -> Result<(), FederatedAggregateAnomalyError> {
    if !safe_text(&request.objective)
        || !safe_text(&request.capability_id)
        || !safe_text(&request.benchmark_world)
        || !safe_text(&request.metric_name)
        || !safe_text(&request.protocol_version)
        || request.minimum_sites == 0
        || request.minimum_sites > MAX_SITES
        || request.minimum_replicates_per_site == 0
        || request.minimum_value_milli > request.maximum_value_milli
        || request.maximum_uncertainty_milli == 0
        || request.robust_outlier_threshold_milli == 0
        || request.maximum_temporal_drift_milli == 0
        || request.maximum_suppressed_fraction_milli > 1_000
        || request.observations.is_empty()
        || request.observations.len() > MAX_SITES
    {
        return Err(FederatedAggregateAnomalyError::InvalidRequest(
            "binding, site/replicate floors, range, uncertainty, drift, suppression, and bounded observation inputs are required".into(),
        ));
    }
    let mut site_ids = BTreeSet::new();
    for observation in &request.observations {
        if !safe_text(&observation.site_id)
            || !site_ids.insert(observation.site_id.clone())
            || !safe_text(&observation.study_id)
            || !safe_text(&observation.capability_id)
            || !safe_text(&observation.benchmark_world)
            || !safe_text(&observation.metric_name)
            || !safe_text(&observation.protocol_version)
            || observation.uncertainty_milli == 0
            || observation.replicate_count == 0
            || !observation.local_only
            || observation.contains_human_data
            || observation.contains_direct_identifiers
        {
            return Err(FederatedAggregateAnomalyError::InvalidRequest(format!(
                "site {} has invalid aggregate-only identity, quality, protocol, or privacy fields",
                observation.site_id
            )));
        }
    }
    Ok(())
}

pub fn analyze_federated_aggregate_anomalies(
    request: &FederatedAggregateAnomalyRequest,
) -> Result<FederatedAggregateAnomalyAssessment, FederatedAggregateAnomalyError> {
    validate_request(request)?;
    let mut candidate = request
        .observations
        .iter()
        .filter(|observation| {
            observation.capability_id == request.capability_id
                && observation.benchmark_world == request.benchmark_world
                && observation.metric_name == request.metric_name
                && observation.model_system == request.model_system
                && observation.replicate_count >= request.minimum_replicates_per_site
        })
        .collect::<Vec<_>>();
    let mut excluded = request
        .observations
        .iter()
        .filter(|observation| {
            !candidate
                .iter()
                .any(|item| item.site_id == observation.site_id)
        })
        .map(|observation| observation.site_id.clone())
        .collect::<Vec<_>>();
    candidate.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    excluded.sort();
    let candidate_site_order = candidate
        .iter()
        .map(|observation| observation.site_id.clone())
        .collect::<Vec<_>>();
    let values = candidate
        .iter()
        .map(|observation| observation.aggregate_value_milli)
        .collect::<Vec<_>>();
    let robust_median = if values.is_empty() {
        0
    } else {
        median(values.clone())
    };
    let deviations = values
        .iter()
        .map(|value| value.abs_diff(robust_median))
        .collect::<Vec<_>>();
    let mad = if deviations.is_empty() {
        0
    } else {
        let mut deviations = deviations;
        deviations.sort_unstable();
        deviations[deviations.len() / 2]
    };
    let robust_threshold = request
        .robust_outlier_threshold_milli
        .max(mad.saturating_mul(3));
    let mut records = Vec::new();
    let mut anomalies = Vec::new();
    let mut reviews = Vec::new();
    let mut clean = Vec::new();
    let mut suppressed = 0usize;
    let mut uncertainty = Vec::new();
    let mut negative = Vec::new();
    for observation in &candidate {
        let deviation = observation.aggregate_value_milli.abs_diff(robust_median);
        let temporal_drift = observation
            .historical_value_milli
            .map(|historical| observation.aggregate_value_milli.abs_diff(historical))
            .unwrap_or(0);
        let mut kinds = Vec::new();
        if observation.aggregate_value_milli < request.minimum_value_milli
            || observation.aggregate_value_milli > request.maximum_value_milli
        {
            kinds.push(AggregateAnomalyKind::ImpossibleValue);
        }
        if deviation > robust_threshold {
            kinds.push(AggregateAnomalyKind::RobustOutlier);
        }
        if temporal_drift > request.maximum_temporal_drift_milli {
            kinds.push(AggregateAnomalyKind::TemporalDrift);
        }
        if observation.protocol_version != request.protocol_version {
            kinds.push(AggregateAnomalyKind::ProtocolMismatch);
        }
        if observation.privacy_suppressed {
            kinds.push(AggregateAnomalyKind::PrivacySuppressed);
            suppressed = suppressed.saturating_add(1);
        }
        if observation.uncertainty_milli > request.maximum_uncertainty_milli {
            kinds.push(AggregateAnomalyKind::HighUncertainty);
        }
        if observation.replicate_count < request.minimum_replicates_per_site {
            kinds.push(AggregateAnomalyKind::UnderReplicated);
        }
        kinds.sort();
        kinds.dedup();
        let review_required = !kinds.is_empty();
        if review_required {
            anomalies.push(observation.site_id.clone());
            reviews.push(observation.site_id.clone());
            negative.push(format!(
                "{} requires local review for {} aggregate anomaly signal(s)",
                observation.site_id,
                kinds.len()
            ));
        } else {
            clean.push(observation.site_id.clone());
        }
        if observation.uncertainty_milli > request.maximum_uncertainty_milli {
            uncertainty.push(format!(
                "{} uncertainty exceeds the declared review ceiling",
                observation.site_id
            ));
        }
        let explanation = if review_required {
            "aggregate signal is flagged for site-local protocol/data review; no automatic exclusion is applied".into()
        } else {
            "no declared aggregate anomaly gate was crossed".into()
        };
        records.push(AggregateAnomalyRecord {
            site_id: observation.site_id.clone(),
            deviation_from_median_milli: deviation,
            temporal_drift_milli: temporal_drift,
            kinds,
            review_required,
            explanation,
        });
    }
    let suppressed_fraction = if candidate.is_empty() {
        1_000
    } else {
        ((suppressed.saturating_mul(1_000)) / candidate.len()).min(1_000) as u16
    };
    if candidate.len() < request.minimum_sites {
        negative.push("binding-matched site count is below the consortium floor".into());
    }
    if suppressed_fraction > request.maximum_suppressed_fraction_milli {
        negative.push(
            "privacy suppression exceeds the declared anomaly-detection coverage floor".into(),
        );
    }
    if !excluded.is_empty() {
        uncertainty.push("binding-mismatched or under-replicated sites were excluded without requesting raw data".into());
    }
    if candidate
        .iter()
        .any(|observation| observation.protocol_version != request.protocol_version)
    {
        uncertainty.push(
            "protocol-version mismatch is a review signal; site causes remain unobserved".into(),
        );
    }
    negative.sort();
    negative.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    anomalies.sort();
    reviews.sort();
    clean.sort();
    let disposition = if candidate.len() < request.minimum_sites {
        AggregateAnomalyDisposition::Insufficient
    } else if suppressed_fraction > request.maximum_suppressed_fraction_milli {
        AggregateAnomalyDisposition::Blocked
    } else if !anomalies.is_empty() {
        AggregateAnomalyDisposition::Review
    } else {
        AggregateAnomalyDisposition::Clean
    };
    let mut assessment = FederatedAggregateAnomalyAssessment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        capability_id: request.capability_id.clone(),
        benchmark_world: request.benchmark_world.clone(),
        metric_name: request.metric_name.clone(),
        candidate_site_order,
        excluded_site_order: excluded,
        anomaly_order: anomalies,
        review_order: reviews,
        clean_order: clean,
        records,
        robust_median_milli: robust_median,
        median_absolute_deviation_milli: mad,
        robust_threshold_milli: robust_threshold,
        suppressed_fraction_milli: suppressed_fraction,
        disposition,
        negative_evidence: negative,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-aggregate-anomaly"),
    };
    assessment.digest = ContentHash::of_value(&output_body(&assessment))
        .map_err(|error| FederatedAggregateAnomalyError::Digest(error.to_string()))?;
    assessment.validate()?;
    Ok(assessment)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(site_id: &str, value: i64) -> FederatedAggregateObservation {
        FederatedAggregateObservation {
            site_id: site_id.into(),
            study_id: format!("study-{site_id}"),
            capability_id: "segmentation".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "dice".into(),
            model_system: GliomaModelSystem::Organoid,
            aggregate_value_milli: value,
            uncertainty_milli: 20,
            replicate_count: 5,
            historical_value_milli: Some(value),
            protocol_version: "dice".into(),
            privacy_suppressed: false,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            source_digest: ContentHash::of_bytes(site_id.as_bytes()),
        }
    }

    fn request(
        observations: Vec<FederatedAggregateObservation>,
    ) -> FederatedAggregateAnomalyRequest {
        FederatedAggregateAnomalyRequest {
            objective: "audit a glioma benchmark".into(),
            capability_id: "segmentation".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "dice".into(),
            protocol_version: "dice".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 2,
            minimum_replicates_per_site: 3,
            minimum_value_milli: 0,
            maximum_value_milli: 1_000,
            maximum_uncertainty_milli: 100,
            robust_outlier_threshold_milli: 100,
            maximum_temporal_drift_milli: 100,
            maximum_suppressed_fraction_milli: 500,
            observations,
        }
    }

    #[test]
    fn clean_aggregate_window_is_not_overflagged() {
        let assessment = analyze_federated_aggregate_anomalies(&request(vec![
            observation("site-a", 700),
            observation("site-b", 710),
            observation("site-c", 690),
        ]))
        .unwrap();
        assert_eq!(assessment.disposition, AggregateAnomalyDisposition::Clean);
        assert!(assessment.anomaly_order.is_empty());
        assert!(assessment.validate().is_ok());
    }

    #[test]
    fn outlier_and_temporal_shift_route_to_review() {
        let mut shifted = observation("site-b", 950);
        shifted.historical_value_milli = Some(700);
        let assessment = analyze_federated_aggregate_anomalies(&request(vec![
            observation("site-a", 700),
            shifted,
            observation("site-c", 710),
        ]))
        .unwrap();
        let record = assessment
            .records
            .iter()
            .find(|record| record.site_id == "site-b")
            .unwrap();
        assert!(record.review_required);
        assert!(record.kinds.contains(&AggregateAnomalyKind::RobustOutlier));
        assert!(record.kinds.contains(&AggregateAnomalyKind::TemporalDrift));
        assert_eq!(assessment.disposition, AggregateAnomalyDisposition::Review);
    }

    #[test]
    fn privacy_suppression_blocks_without_excluding_a_site() {
        let mut suppressed = observation("site-b", 705);
        suppressed.privacy_suppressed = true;
        let mut request = request(vec![observation("site-a", 700), suppressed]);
        request.maximum_suppressed_fraction_milli = 0;
        let assessment = analyze_federated_aggregate_anomalies(&request).unwrap();
        assert_eq!(assessment.disposition, AggregateAnomalyDisposition::Blocked);
        assert!(assessment.excluded_site_order.is_empty());
        assert!(assessment.review_order.contains(&"site-b".to_string()));
    }

    #[test]
    fn binding_mismatch_is_explicitly_excluded() {
        let mut mismatch = observation("site-b", 700);
        mismatch.metric_name = "wrong".into();
        let assessment = analyze_federated_aggregate_anomalies(&request(vec![
            observation("site-a", 700),
            mismatch,
        ]))
        .unwrap();
        assert_eq!(assessment.excluded_site_order, vec!["site-b"]);
        assert!(assessment
            .uncertainty
            .iter()
            .any(|item| item.contains("excluded")));
    }

    #[test]
    fn deterministic_digest_repeats() {
        let input = request(vec![observation("site-a", 700), observation("site-b", 710)]);
        let left = analyze_federated_aggregate_anomalies(&input).unwrap();
        let right = analyze_federated_aggregate_anomalies(&input).unwrap();
        assert_eq!(left, right);
    }
}
