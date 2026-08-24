//! Deterministic quality-metric drift sentinel for continual multimodal ingestion.
//!
//! Atlas feature: `AFA-adapter-P07-F02`.
//!
//! The sentinel compares typed QC summaries against a content-addressed
//! baseline. It does not inspect raw bytes and never turns an unmeasured metric
//! into stability; drift and unknown states remain explicit release inputs.

use bioprism_foundation::{
    CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState, LossSeverity,
    ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P07-F02";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriftMetric {
    pub metric_id: String,
    pub baseline: f64,
    pub current: Option<f64>,
    pub tolerance: f64,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityDriftPolicy {
    pub require_conformance: bool,
    pub minimum_measured_fraction: f64,
}

impl Default for QualityDriftPolicy {
    fn default() -> Self {
        Self {
            require_conformance: true,
            minimum_measured_fraction: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityDriftRequest {
    pub dataset_id: String,
    pub modality: String,
    pub source_digest: ContentHash,
    pub baseline_digest: ContentHash,
    pub metrics: Vec<DriftMetric>,
    pub policy: QualityDriftPolicy,
    pub conformance_verified: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftMetricStatus {
    Stable,
    Drifted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriftMetricResult {
    pub metric_id: String,
    pub status: DriftMetricStatus,
    pub delta: Option<f64>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftDisposition {
    Stable,
    Drifted,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityDriftSummary {
    pub disposition: DriftDisposition,
    pub measured: usize,
    pub stable: usize,
    pub drifted: usize,
    pub unknown: usize,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityDriftReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub dataset_id: String,
    pub modality: String,
    pub request_digest: ContentHash,
    pub summary: QualityDriftSummary,
    pub metrics: Vec<DriftMetricResult>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl QualityDriftReceipt {
    pub fn validate(&self) -> Result<(), QualityDriftError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.feature_id != FEATURE_ID
        {
            return Err(QualityDriftError::InvalidField("schema or feature".into()));
        }
        if self.dataset_id.trim().is_empty()
            || self.modality.trim().is_empty()
            || self.metrics.is_empty()
        {
            return Err(QualityDriftError::InvalidField(
                "identity and metrics are required".into(),
            ));
        }
        if self.metrics.len() != self.summary.stable + self.summary.drifted + self.summary.unknown {
            return Err(QualityDriftError::InvalidField(
                "metric status counts".into(),
            ));
        }
        if self.summary.reasons.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(QualityDriftError::InvalidField(
                "reasons, locality, and boundary are required".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| QualityDriftError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, QualityDriftError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| QualityDriftError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| QualityDriftError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum QualityDriftError {
    #[error("invalid quality-drift field: {0}")]
    InvalidField(String),
    #[error("duplicate quality metric {0}")]
    DuplicateMetric(String),
    #[error("invalid quality-drift measurement: {0}")]
    InvalidMeasurement(String),
    #[error("artifact error: {0}")]
    Artifact(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn quality_drift_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["multimodal data engineer".into(), "quality operator".into()].into(),
        behavior: "compares measured modality QC metrics against a signed baseline and emits stable, drifted, unknown, or blocked states".into(),
        value: "catches continual-ingestion quality drift before stale or degraded data enters a research workflow".into(),
        inputs: vec![TypedPort { name: "quality_drift_request".into(), schema: "QualityDriftRequest@1".into(), required: true }],
        outputs: vec![TypedPort { name: "quality_drift_receipt".into(), schema: "QualityDriftReceipt@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:institution-local-qc".into(), "write:local-drift-receipt".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "fixture:adapter-quality-drift".into(), state: EvidenceState::Supported, locator: Some("fixtures/quality-drift".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: bioprism_foundation::AutonomyTier::A0,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn evaluate_quality_drift(
    request: &QualityDriftRequest,
) -> Result<QualityDriftReceipt, QualityDriftError> {
    validate_request(request)?;
    let request_value = serde_json::to_value(request)
        .map_err(|error| QualityDriftError::Serialization(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| QualityDriftError::Serialization(error.to_string()))?;
    let mut stable: usize = 0;
    let mut drifted: usize = 0;
    let mut unknown: usize = 0;
    let mut reasons = Vec::new();
    let metrics = request
        .metrics
        .iter()
        .map(|metric| {
            let (status, delta, row_reasons) = match metric.current {
                None => (
                    DriftMetricStatus::Unknown,
                    None,
                    vec![format!("metric {} is unmeasured", metric.metric_id)],
                ),
                Some(current) => {
                    let delta = current - metric.baseline;
                    if delta.abs() > metric.tolerance {
                        (
                            DriftMetricStatus::Drifted,
                            Some(delta),
                            vec![format!(
                                "metric {} drifted by {} beyond tolerance {}",
                                metric.metric_id, delta, metric.tolerance
                            )],
                        )
                    } else {
                        (DriftMetricStatus::Stable, Some(delta), Vec::new())
                    }
                }
            };
            match status {
                DriftMetricStatus::Stable => stable += 1,
                DriftMetricStatus::Drifted => {
                    drifted += 1;
                    reasons.extend(row_reasons.clone());
                }
                DriftMetricStatus::Unknown => {
                    unknown += 1;
                    reasons.extend(row_reasons.clone());
                }
            }
            DriftMetricResult {
                metric_id: metric.metric_id.clone(),
                status,
                delta,
                reasons: row_reasons,
            }
        })
        .collect::<Vec<_>>();
    if request.policy.require_conformance && !request.conformance_verified {
        reasons.push("adapter conformance is not verified".into());
    }
    let measured_fraction = stable.saturating_add(drifted) as f64 / request.metrics.len() as f64;
    if measured_fraction < request.policy.minimum_measured_fraction {
        reasons.push(format!(
            "measured metric fraction {measured_fraction:.6} is below required {}",
            request.policy.minimum_measured_fraction
        ));
    }
    if !request.raw_data_local {
        reasons.push("raw research data is not local to the institution".into());
    }
    let disposition = if !request.raw_data_local
        || (request.policy.require_conformance && !request.conformance_verified)
    {
        DriftDisposition::Blocked
    } else if drifted > 0 {
        DriftDisposition::Drifted
    } else if measured_fraction < request.policy.minimum_measured_fraction || unknown > 0 {
        DriftDisposition::Unknown
    } else {
        DriftDisposition::Stable
    };
    if reasons.is_empty() {
        reasons.push("all required QC metrics remain within baseline tolerance".into());
    }
    let summary = QualityDriftSummary {
        disposition,
        measured: stable + drifted,
        stable,
        drifted,
        unknown,
        reasons,
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "dataset_id": request.dataset_id,
        "modality": request.modality,
        "source_digest": request.source_digest,
        "baseline_digest": request.baseline_digest,
        "request_digest": request_digest,
        "summary": summary,
        "metrics": metrics,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("quality-drift:{}", request.dataset_id),
        "application/vnd.aurora.quality-drift+json",
        &payload,
        vec![SemanticLoss { field: "raw_data".into(), reason: "raw experimental data remains institution-local; receipt exports QC values and digests only".into(), severity: LossSeverity::Bounded }],
        vec![ProvenanceLink { source_id: request.dataset_id.clone(), relation: "quality-drift-source-digest".into(), digest: request.source_digest.clone() }, ProvenanceLink { source_id: "baseline".into(), relation: "quality-drift-baseline-digest".into(), digest: request.baseline_digest.clone() }],
    ).map_err(|error| QualityDriftError::Artifact(error.to_string()))?;
    let receipt = QualityDriftReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        dataset_id: request.dataset_id.clone(),
        modality: request.modality.clone(),
        request_digest,
        summary,
        metrics,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &QualityDriftRequest) -> Result<(), QualityDriftError> {
    if request.dataset_id.trim().is_empty()
        || request.modality.trim().is_empty()
        || request.metrics.is_empty()
    {
        return Err(QualityDriftError::InvalidField(
            "dataset, modality, and metrics are required".into(),
        ));
    }
    if !request.policy.minimum_measured_fraction.is_finite()
        || !(0.0..=1.0).contains(&request.policy.minimum_measured_fraction)
    {
        return Err(QualityDriftError::InvalidMeasurement(
            "minimum_measured_fraction must be within 0..=1".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for metric in &request.metrics {
        if metric.metric_id.trim().is_empty() || !ids.insert(metric.metric_id.clone()) {
            return Err(QualityDriftError::DuplicateMetric(metric.metric_id.clone()));
        }
        if !metric.baseline.is_finite()
            || metric.current.is_some_and(|value| !value.is_finite())
            || !metric.tolerance.is_finite()
            || metric.tolerance < 0.0
        {
            return Err(QualityDriftError::InvalidMeasurement(
                metric.metric_id.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }
    fn request() -> QualityDriftRequest {
        QualityDriftRequest {
            dataset_id: "dataset:drift".into(),
            modality: "image".into(),
            source_digest: hash("source"),
            baseline_digest: hash("baseline"),
            metrics: vec![
                DriftMetric {
                    metric_id: "snr".into(),
                    baseline: 10.0,
                    current: Some(10.2),
                    tolerance: 0.5,
                    required: true,
                },
                DriftMetric {
                    metric_id: "focus".into(),
                    baseline: 0.8,
                    current: Some(0.4),
                    tolerance: 0.1,
                    required: true,
                },
            ],
            policy: QualityDriftPolicy::default(),
            conformance_verified: true,
            raw_data_local: true,
        }
    }
    #[test]
    fn drift_is_detected_without_raw_data_access() {
        let receipt = evaluate_quality_drift(&request()).unwrap();
        assert_eq!(receipt.summary.disposition, DriftDisposition::Drifted);
        assert_eq!(receipt.summary.drifted, 1);
    }
    #[test]
    fn missing_metric_is_unknown_not_stable() {
        let mut request = request();
        request.metrics[1].current = None;
        let receipt = evaluate_quality_drift(&request).unwrap();
        assert_eq!(receipt.summary.disposition, DriftDisposition::Unknown);
        assert_eq!(receipt.summary.unknown, 1);
    }
    #[test]
    fn unverified_conformance_blocks_release() {
        let mut request = request();
        request.conformance_verified = false;
        let receipt = evaluate_quality_drift(&request).unwrap();
        assert_eq!(receipt.summary.disposition, DriftDisposition::Blocked);
    }
}
