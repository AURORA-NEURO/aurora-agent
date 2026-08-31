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
pub const CONTRACT_VERSION: &str = "quality-drift/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_METRICS: usize = 4096;
const MAX_REASON_ITEMS: usize = 8192;

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
    pub baseline: f64,
    pub current: Option<f64>,
    pub tolerance: f64,
    pub required: bool,
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
    pub contract_version: String,
    pub feature_id: String,
    pub dataset_id: String,
    pub modality: String,
    pub source_digest: ContentHash,
    pub baseline_digest: ContentHash,
    pub request_digest: ContentHash,
    pub policy: QualityDriftPolicy,
    pub conformance_verified: bool,
    pub summary: QualityDriftSummary,
    pub metrics: Vec<DriftMetricResult>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl QualityDriftReceipt {
    pub fn validate(&self) -> Result<(), QualityDriftError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(QualityDriftError::InvalidField("schema or feature".into()));
        }
        if self.dataset_id.trim().is_empty()
            || self.modality.trim().is_empty()
            || self.metrics.is_empty()
            || self.metrics.len() > MAX_METRICS
            || self.request_digest == ContentHash::of_bytes(b"")
            || self.source_digest == ContentHash::of_bytes(b"")
            || self.baseline_digest == ContentHash::of_bytes(b"")
        {
            return Err(QualityDriftError::InvalidField(
                "identity and metrics are required".into(),
            ));
        }
        if self.metrics.len() != self.summary.stable + self.summary.drifted + self.summary.unknown
            || self.summary.measured != self.summary.stable + self.summary.drifted
        {
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
        validate_text("dataset_id", &self.dataset_id)?;
        validate_text("modality", &self.modality)?;
        validate_policy(&self.policy)?;
        validate_reasons(&self.summary.reasons, "summary.reasons")?;
        let mut metric_ids = BTreeSet::new();
        for metric in &self.metrics {
            validate_text("metric_id", &metric.metric_id)?;
            if !metric_ids.insert(metric.metric_id.clone()) {
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
            let expected = derive_metric_result(&DriftMetric {
                metric_id: metric.metric_id.clone(),
                baseline: metric.baseline,
                current: metric.current,
                tolerance: metric.tolerance,
                required: metric.required,
            });
            if *metric != expected {
                return Err(QualityDriftError::InvalidField(format!(
                    "metric {} is not derived from its baseline and observation",
                    metric.metric_id
                )));
            }
            if metric.delta.is_some_and(|delta| !delta.is_finite()) {
                return Err(QualityDriftError::InvalidMeasurement(
                    metric.metric_id.clone(),
                ));
            }
            if matches!(
                metric.status,
                DriftMetricStatus::Drifted | DriftMetricStatus::Unknown
            ) && metric.reasons.is_empty()
            {
                return Err(QualityDriftError::InvalidField(format!(
                    "metric {} has no witness reason",
                    metric.metric_id
                )));
            }
        }
        for metric in &self.metrics {
            validate_reasons(&metric.reasons, "metric.reasons")?;
        }
        if self
            .metrics
            .windows(2)
            .any(|pair| pair[0].metric_id >= pair[1].metric_id)
        {
            return Err(QualityDriftError::InvalidField(
                "metric results must be strictly sorted".into(),
            ));
        }
        let request = QualityDriftRequest {
            dataset_id: self.dataset_id.clone(),
            modality: self.modality.clone(),
            source_digest: self.source_digest.clone(),
            baseline_digest: self.baseline_digest.clone(),
            metrics: self
                .metrics
                .iter()
                .map(|metric| DriftMetric {
                    metric_id: metric.metric_id.clone(),
                    baseline: metric.baseline,
                    current: metric.current,
                    tolerance: metric.tolerance,
                    required: metric.required,
                })
                .collect(),
            policy: self.policy.clone(),
            conformance_verified: self.conformance_verified,
            raw_data_local: self.raw_data_local,
        };
        let expected_request_digest = request_digest(&request)?;
        if self.request_digest != expected_request_digest {
            return Err(QualityDriftError::InvalidField(
                "request digest does not match the retained drift inputs".into(),
            ));
        }
        let expected_summary = derive_summary(
            &self.metrics,
            &self.policy,
            self.conformance_verified,
            self.raw_data_local,
        );
        if self.summary != expected_summary {
            return Err(QualityDriftError::InvalidField(
                "drift summary is not derived from metric and policy state".into(),
            ));
        }
        if self.semantic_loss != quality_drift_semantic_loss() {
            return Err(QualityDriftError::InvalidField(
                "quality-drift semantic loss is not canonical".into(),
            ));
        }
        let expected_provenance = vec![
            ProvenanceLink {
                source_id: self.dataset_id.clone(),
                relation: "quality-drift-source-digest".into(),
                digest: self.source_digest.clone(),
            },
            ProvenanceLink {
                source_id: "baseline".into(),
                relation: "quality-drift-baseline-digest".into(),
                digest: self.baseline_digest.clone(),
            },
        ];
        if self.artifact.artifact_id != format!("quality-drift:{}", self.dataset_id)
            || self.artifact.content_type != "application/vnd.aurora.quality-drift+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != expected_provenance
        {
            return Err(QualityDriftError::Artifact(
                "quality-drift artifact is not bound to its retained inputs".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| QualityDriftError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&quality_drift_payload(self))
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

fn validate_text(field: &str, value: &str) -> Result<(), QualityDriftError> {
    if value.is_empty() || value.trim() != value {
        return Err(QualityDriftError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(QualityDriftError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_reasons(values: &[String], field: &str) -> Result<(), QualityDriftError> {
    if values.len() > MAX_REASON_ITEMS {
        return Err(QualityDriftError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    for value in values {
        validate_text(field, value)?;
    }
    Ok(())
}

fn validate_policy(policy: &QualityDriftPolicy) -> Result<(), QualityDriftError> {
    if !policy.minimum_measured_fraction.is_finite()
        || !(0.0..=1.0).contains(&policy.minimum_measured_fraction)
    {
        return Err(QualityDriftError::InvalidMeasurement(
            "minimum_measured_fraction must be within 0..=1".into(),
        ));
    }
    Ok(())
}

fn derive_metric_result(metric: &DriftMetric) -> DriftMetricResult {
    let (status, delta, reasons) = match metric.current {
        None => (
            DriftMetricStatus::Unknown,
            None,
            vec![format!(
                "metric {} is {}unmeasured",
                metric.metric_id,
                if metric.required {
                    "required and "
                } else {
                    "optional and "
                }
            )],
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
    DriftMetricResult {
        metric_id: metric.metric_id.clone(),
        baseline: metric.baseline,
        current: metric.current,
        tolerance: metric.tolerance,
        required: metric.required,
        status,
        delta,
        reasons,
    }
}

fn derive_summary(
    metrics: &[DriftMetricResult],
    policy: &QualityDriftPolicy,
    conformance_verified: bool,
    raw_data_local: bool,
) -> QualityDriftSummary {
    let stable = metrics
        .iter()
        .filter(|metric| metric.status == DriftMetricStatus::Stable)
        .count();
    let drifted = metrics
        .iter()
        .filter(|metric| metric.status == DriftMetricStatus::Drifted)
        .count();
    let unknown = metrics
        .iter()
        .filter(|metric| metric.status == DriftMetricStatus::Unknown)
        .count();
    let mut reasons = metrics
        .iter()
        .filter(|metric| {
            matches!(
                metric.status,
                DriftMetricStatus::Drifted | DriftMetricStatus::Unknown
            )
        })
        .flat_map(|metric| metric.reasons.iter().cloned())
        .collect::<Vec<_>>();
    if policy.require_conformance && !conformance_verified {
        reasons.push("adapter conformance is not verified".into());
    }
    let measured_fraction = stable.saturating_add(drifted) as f64 / metrics.len() as f64;
    if measured_fraction < policy.minimum_measured_fraction {
        reasons.push(format!(
            "measured metric fraction {measured_fraction:.6} is below required {}",
            policy.minimum_measured_fraction
        ));
    }
    if !raw_data_local {
        reasons.push("raw research data is not local to the institution".into());
    }
    let disposition = if !raw_data_local || (policy.require_conformance && !conformance_verified) {
        DriftDisposition::Blocked
    } else if drifted > 0 {
        DriftDisposition::Drifted
    } else if measured_fraction < policy.minimum_measured_fraction || unknown > 0 {
        DriftDisposition::Unknown
    } else {
        DriftDisposition::Stable
    };
    if reasons.is_empty() {
        reasons.push("all required QC metrics remain within baseline tolerance".into());
    }
    QualityDriftSummary {
        disposition,
        measured: stable + drifted,
        stable,
        drifted,
        unknown,
        reasons,
    }
}

fn quality_drift_semantic_loss() -> Vec<SemanticLoss> {
    vec![SemanticLoss {
        field: "raw_data".into(),
        reason: "raw experimental data remains institution-local; receipt exports QC values and digests only".into(),
        severity: LossSeverity::Bounded,
    }]
}

fn request_digest(request: &QualityDriftRequest) -> Result<ContentHash, QualityDriftError> {
    let mut canonical = request.clone();
    canonical
        .metrics
        .sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    let value = serde_json::to_value(&canonical)
        .map_err(|error| QualityDriftError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| QualityDriftError::Serialization(error.to_string()))
}

fn quality_drift_payload(receipt: &QualityDriftReceipt) -> serde_json::Value {
    quality_drift_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.dataset_id,
        &receipt.modality,
        &receipt.source_digest,
        &receipt.baseline_digest,
        &receipt.request_digest,
        &receipt.policy,
        receipt.conformance_verified,
        &receipt.summary,
        &receipt.metrics,
        &receipt.semantic_loss,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn quality_drift_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    dataset_id: &str,
    modality: &str,
    source_digest: &ContentHash,
    baseline_digest: &ContentHash,
    request_digest: &ContentHash,
    policy: &QualityDriftPolicy,
    conformance_verified: bool,
    summary: &QualityDriftSummary,
    metrics: &[DriftMetricResult],
    semantic_loss: &[SemanticLoss],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "dataset_id": dataset_id,
        "modality": modality,
        "source_digest": source_digest,
        "baseline_digest": baseline_digest,
        "request_digest": request_digest,
        "policy": policy,
        "conformance_verified": conformance_verified,
        "summary": summary,
        "metrics": metrics,
        "semantic_loss": semantic_loss,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    let request_digest = request_digest(request)?;
    let mut ordered_metrics = request.metrics.clone();
    ordered_metrics.sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    let metrics = ordered_metrics
        .iter()
        .map(derive_metric_result)
        .collect::<Vec<_>>();
    let summary = derive_summary(
        &metrics,
        &request.policy,
        request.conformance_verified,
        request.raw_data_local,
    );
    let semantic_loss = quality_drift_semantic_loss();
    let provenance = vec![
        ProvenanceLink {
            source_id: request.dataset_id.clone(),
            relation: "quality-drift-source-digest".into(),
            digest: request.source_digest.clone(),
        },
        ProvenanceLink {
            source_id: "baseline".into(),
            relation: "quality-drift-baseline-digest".into(),
            digest: request.baseline_digest.clone(),
        },
    ];
    let payload = quality_drift_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &request.dataset_id,
        &request.modality,
        &request.source_digest,
        &request.baseline_digest,
        &request_digest,
        &request.policy,
        request.conformance_verified,
        &summary,
        &metrics,
        &semantic_loss,
        &provenance,
        request.raw_data_local,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("quality-drift:{}", request.dataset_id),
        "application/vnd.aurora.quality-drift+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| QualityDriftError::Artifact(error.to_string()))?;
    let receipt = QualityDriftReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        dataset_id: request.dataset_id.clone(),
        modality: request.modality.clone(),
        source_digest: request.source_digest.clone(),
        baseline_digest: request.baseline_digest.clone(),
        request_digest,
        policy: request.policy.clone(),
        conformance_verified: request.conformance_verified,
        summary,
        metrics,
        semantic_loss,
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
        || request.metrics.len() > MAX_METRICS
    {
        return Err(QualityDriftError::InvalidField(
            "dataset, modality, and metrics are required".into(),
        ));
    }
    validate_text("dataset_id", &request.dataset_id)?;
    validate_text("modality", &request.modality)?;
    if request.source_digest == ContentHash::of_bytes(b"")
        || request.baseline_digest == ContentHash::of_bytes(b"")
    {
        return Err(QualityDriftError::InvalidField(
            "source and baseline digests must be non-empty".into(),
        ));
    }
    validate_policy(&request.policy)?;
    let mut ids = BTreeSet::new();
    for metric in &request.metrics {
        if metric.metric_id.trim().is_empty() || !ids.insert(metric.metric_id.clone()) {
            return Err(QualityDriftError::DuplicateMetric(metric.metric_id.clone()));
        }
        validate_text("metric_id", &metric.metric_id)?;
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
        assert!(receipt.metrics[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("required")));
    }
    #[test]
    fn unverified_conformance_blocks_release() {
        let mut request = request();
        request.conformance_verified = false;
        let receipt = evaluate_quality_drift(&request).unwrap();
        assert_eq!(receipt.summary.disposition, DriftDisposition::Blocked);
    }

    #[test]
    fn metric_results_are_canonical_and_receipt_counts_are_bound() {
        let receipt = evaluate_quality_drift(&request()).unwrap();
        assert_eq!(
            receipt
                .metrics
                .iter()
                .map(|metric| metric.metric_id.as_str())
                .collect::<Vec<_>>(),
            vec!["focus", "snr"]
        );

        let mut tampered = receipt;
        tampered.summary.measured += 1;
        assert!(tampered.validate().is_err());
    }

    #[test]
    fn empty_source_or_baseline_digest_is_rejected() {
        let mut input = request();
        input.source_digest = ContentHash::of_bytes(b"");
        assert!(evaluate_quality_drift(&input).is_err());
    }

    #[test]
    fn metric_input_order_is_canonicalized_in_the_receipt() {
        let mut reversed = request();
        reversed.metrics.reverse();
        let left = evaluate_quality_drift(&request()).unwrap();
        let right = evaluate_quality_drift(&reversed).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.request_digest, right.request_digest);
    }

    #[test]
    fn observed_metric_tampering_is_rejected() {
        let mut receipt = evaluate_quality_drift(&request()).unwrap();
        receipt.metrics[0].current = Some(0.9);
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn drift_artifact_payload_tampering_is_rejected() {
        let mut receipt = evaluate_quality_drift(&request()).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }
}
