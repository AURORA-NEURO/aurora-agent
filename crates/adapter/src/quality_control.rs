//! Multimodal quality-control receipt for research data adapters.
//!
//! Atlas feature: `AFA-adapter-P07-F01`.
//!
//! A conformance-verified adapter is necessary but not sufficient for downstream analysis. This
//! module evaluates named modality metrics against declared gates, keeps missing measurements
//! explicitly unknown, and refuses to certify a dataset when raw data has left the institution.
//! The exported artifact contains typed metrics and digests only.

use bioprism_foundation::{
    CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState, LossSeverity,
    ProvenanceLink, ResearchContractError, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P07-F01";
pub const FEATURE_VERSION: &str = "0.1.0";
pub const CONTRACT_VERSION: &str = "quality-control/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_METRICS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricDirection {
    AtLeast,
    AtMost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    Pass,
    Warning,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityMetric {
    pub metric_id: String,
    pub value: Option<f64>,
    pub threshold: f64,
    pub direction: MetricDirection,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityControlRequest {
    pub dataset_id: String,
    pub modality: String,
    pub source_digest: ContentHash,
    pub metrics: Vec<QualityMetric>,
    pub conformance_verified: bool,
    pub raw_data_local: bool,
}

impl QualityControlRequest {
    pub fn validate(&self) -> Result<(), QualityControlError> {
        if self.dataset_id.trim().is_empty() || self.modality.trim().is_empty() {
            return Err(QualityControlError::InvalidField(
                "dataset_id and modality are required".into(),
            ));
        }
        if self.metrics.is_empty() {
            return Err(QualityControlError::InvalidField(
                "at least one quality metric is required".into(),
            ));
        }
        if self.metrics.len() > MAX_METRICS {
            return Err(QualityControlError::InvalidField(
                "quality metric count exceeds the bounded contract".into(),
            ));
        }
        validate_text("dataset_id", &self.dataset_id)?;
        validate_text("modality", &self.modality)?;
        if self.source_digest == ContentHash::of_bytes(b"") {
            return Err(QualityControlError::InvalidField(
                "source digest is required".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        for metric in &self.metrics {
            validate_text("metric_id", &metric.metric_id)?;
            if !ids.insert(metric.metric_id.clone()) {
                return Err(QualityControlError::InvalidField(
                    "metric identifiers must be unique and non-empty".into(),
                ));
            }
            if !metric.threshold.is_finite() || metric.value.is_some_and(|value| !value.is_finite())
            {
                return Err(QualityControlError::InvalidField(
                    "metric threshold and value must be finite".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityDisposition {
    Pass,
    PassWithWarnings,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityControlSummary {
    pub disposition: QualityDisposition,
    pub passed: usize,
    pub warnings: usize,
    pub blocked: usize,
    pub unknown: usize,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub dataset_id: String,
    pub modality: String,
    pub source_digest: ContentHash,
    pub request_digest: ContentHash,
    pub metrics: Vec<QualityMetric>,
    pub conformance_verified: bool,
    pub summary: QualityControlSummary,
    pub semantic_loss: Vec<SemanticLoss>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl QualityControlReceipt {
    pub fn validate(&self) -> Result<(), QualityControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
        {
            return Err(QualityControlError::Contract(
                ResearchContractError::SchemaVersion {
                    expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                    found: self.schema_version.clone(),
                },
            ));
        }
        if self.feature_id != FEATURE_ID
            || self.dataset_id.trim().is_empty()
            || self.modality.trim().is_empty()
            || self.metrics.is_empty()
            || self.metrics.len() > MAX_METRICS
            || self.source_digest == ContentHash::of_bytes(b"")
            || self.request_digest == ContentHash::of_bytes(b"")
        {
            return Err(QualityControlError::InvalidField(
                "quality-control identity is incomplete".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(QualityControlError::Contract(
                ResearchContractError::BoundaryMismatch {
                    capability: self.dataset_id.clone(),
                },
            ));
        }
        if self.summary.reasons.is_empty() || !self.raw_data_local {
            return Err(QualityControlError::InvalidField(
                "quality receipt requires reasons and local raw data".into(),
            ));
        }
        validate_text("dataset_id", &self.dataset_id)?;
        validate_text("modality", &self.modality)?;
        if self
            .metrics
            .windows(2)
            .any(|pair| pair[0].metric_id >= pair[1].metric_id)
        {
            return Err(QualityControlError::InvalidField(
                "quality metrics must be strictly sorted".into(),
            ));
        }
        let request = QualityControlRequest {
            dataset_id: self.dataset_id.clone(),
            modality: self.modality.clone(),
            source_digest: self.source_digest.clone(),
            metrics: self.metrics.clone(),
            conformance_verified: self.conformance_verified,
            raw_data_local: self.raw_data_local,
        };
        request.validate()?;
        let expected_request_digest = request_digest(&request)?;
        if self.request_digest != expected_request_digest {
            return Err(QualityControlError::InvalidField(
                "request digest does not match the retained quality inputs".into(),
            ));
        }
        let (expected_summary, _) = derive_quality_control(
            &self.metrics,
            self.conformance_verified,
            self.raw_data_local,
        );
        if self.summary != expected_summary {
            return Err(QualityControlError::InvalidField(
                "quality summary is not derived from metric and gate state".into(),
            ));
        }
        if self.semantic_loss != quality_control_semantic_loss() {
            return Err(QualityControlError::InvalidField(
                "quality semantic loss is not canonical".into(),
            ));
        }
        let expected_provenance = vec![ProvenanceLink {
            source_id: self.dataset_id.clone(),
            relation: "quality-control-source-digest".into(),
            digest: self.source_digest.clone(),
        }];
        if self.artifact.artifact_id != format!("quality-control:{}", self.dataset_id)
            || self.artifact.content_type != "application/vnd.aurora.quality-control+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != expected_provenance
        {
            return Err(QualityControlError::InvalidField(
                "quality-control artifact is not bound to its retained inputs".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(QualityControlError::Contract)?;
        self.artifact
            .verify_payload(&quality_control_payload(self))
            .map_err(QualityControlError::Contract)
    }

    pub fn verify_payload(&self, payload: &Value) -> Result<(), QualityControlError> {
        self.validate()?;
        self.artifact
            .verify_payload(payload)
            .map_err(QualityControlError::Contract)
    }

    pub fn digest(&self) -> Result<ContentHash, QualityControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| QualityControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| QualityControlError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum QualityControlError {
    #[error("invalid quality-control request: {0}")]
    InvalidField(String),
    #[error("research contract error: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["multimodal data engineer".into(), "bioinformatician".into()].into(),
        behavior: "evaluates named modality quality metrics against typed gates and emits an explicit pass, warning, blocked, or unknown receipt".into(),
        value: "prevents stale, missing, non-conformant, or externally exposed research data from silently entering analysis".into(),
        inputs: vec![TypedPort { name: "quality_control_request".into(), schema: "QualityControlRequest@1".into(), required: true }],
        outputs: vec![TypedPort { name: "quality_control_receipt".into(), schema: "QualityControlReceipt@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(),
        permissions: ["read:institution-local-dataset".into(), "write:local-qc-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "fixture:adapter-quality-control".into(), state: EvidenceState::Supported, locator: Some("fixtures/quality-control".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: bioprism_foundation::AutonomyTier::A0,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn metric_status(metric: &QualityMetric) -> MetricStatus {
    let Some(value) = metric.value else {
        return MetricStatus::Unknown;
    };
    let passes = match metric.direction {
        MetricDirection::AtLeast => value >= metric.threshold,
        MetricDirection::AtMost => value <= metric.threshold,
    };
    if passes {
        MetricStatus::Pass
    } else if metric.required {
        MetricStatus::Blocked
    } else {
        MetricStatus::Warning
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), QualityControlError> {
    if value.is_empty() || value.trim() != value {
        return Err(QualityControlError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(QualityControlError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn request_digest(request: &QualityControlRequest) -> Result<ContentHash, QualityControlError> {
    let mut canonical = request.clone();
    canonical
        .metrics
        .sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    let value = serde_json::to_value(&canonical)
        .map_err(|error| QualityControlError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| QualityControlError::Serialization(error.to_string()))
}

fn derive_quality_control(
    metrics: &[QualityMetric],
    conformance_verified: bool,
    raw_data_local: bool,
) -> (QualityControlSummary, Vec<Value>) {
    let mut passed = 0;
    let mut warnings = 0;
    let mut blocked = 0;
    let mut unknown = 0;
    let mut reasons = Vec::new();
    let metric_rows = metrics
        .iter()
        .map(|metric| {
            let status = metric_status(metric);
            match status {
                MetricStatus::Pass => passed += 1,
                MetricStatus::Warning => {
                    warnings += 1;
                    reasons.push(format!(
                        "optional metric {} is below its gate",
                        metric.metric_id
                    ));
                }
                MetricStatus::Blocked => {
                    blocked += 1;
                    reasons.push(format!(
                        "required metric {} is below its gate",
                        metric.metric_id
                    ));
                }
                MetricStatus::Unknown => {
                    unknown += 1;
                    reasons.push(format!("metric {} is unmeasured", metric.metric_id));
                }
            }
            json!({
                "metric_id": metric.metric_id,
                "value": metric.value,
                "threshold": metric.threshold,
                "direction": metric.direction,
                "required": metric.required,
                "status": status,
            })
        })
        .collect::<Vec<_>>();
    if !conformance_verified {
        blocked += 1;
        reasons.push("adapter conformance is not verified".into());
    }
    if !raw_data_local {
        blocked += 1;
        reasons.push("raw research data is not local to the institution".into());
    }
    if reasons.is_empty() {
        reasons.push("all required quality gates passed".into());
    }
    let disposition = if blocked > 0 {
        QualityDisposition::Blocked
    } else if unknown > 0 {
        QualityDisposition::Unknown
    } else if warnings > 0 {
        QualityDisposition::PassWithWarnings
    } else {
        QualityDisposition::Pass
    };
    (
        QualityControlSummary {
            disposition,
            passed,
            warnings,
            blocked,
            unknown,
            reasons,
        },
        metric_rows,
    )
}

fn quality_control_semantic_loss() -> Vec<SemanticLoss> {
    vec![SemanticLoss {
        field: "raw_data".into(),
        reason: "raw experimental data remains institution-local; receipt exports metrics and digests only".into(),
        severity: LossSeverity::Bounded,
    }]
}

fn quality_control_payload(receipt: &QualityControlReceipt) -> Value {
    let (_, metric_rows) = derive_quality_control(
        &receipt.metrics,
        receipt.conformance_verified,
        receipt.raw_data_local,
    );
    quality_control_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.dataset_id,
        &receipt.modality,
        &receipt.source_digest,
        &receipt.request_digest,
        receipt.conformance_verified,
        &receipt.summary,
        &metric_rows,
        &receipt.semantic_loss,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn quality_control_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    dataset_id: &str,
    modality: &str,
    source_digest: &ContentHash,
    request_digest: &ContentHash,
    conformance_verified: bool,
    summary: &QualityControlSummary,
    metric_rows: &[Value],
    semantic_loss: &[SemanticLoss],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "dataset_id": dataset_id,
        "modality": modality,
        "source_digest": source_digest,
        "request_digest": request_digest,
        "conformance_verified": conformance_verified,
        "summary": summary,
        "metrics": metric_rows,
        "semantic_loss": semantic_loss,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

pub fn evaluate_quality_control(
    request: &QualityControlRequest,
) -> Result<QualityControlReceipt, QualityControlError> {
    request.validate()?;
    let mut canonical_request = request.clone();
    canonical_request
        .metrics
        .sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    let request_digest = request_digest(&canonical_request)?;
    let (summary, metric_rows) = derive_quality_control(
        &canonical_request.metrics,
        canonical_request.conformance_verified,
        canonical_request.raw_data_local,
    );
    let semantic_loss = quality_control_semantic_loss();
    let provenance = vec![ProvenanceLink {
        source_id: canonical_request.dataset_id.clone(),
        relation: "quality-control-source-digest".into(),
        digest: canonical_request.source_digest.clone(),
    }];
    let payload = quality_control_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &canonical_request.dataset_id,
        &canonical_request.modality,
        &canonical_request.source_digest,
        &request_digest,
        canonical_request.conformance_verified,
        &summary,
        &metric_rows,
        &semantic_loss,
        &provenance,
        canonical_request.raw_data_local,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("quality-control:{}", canonical_request.dataset_id),
        "application/vnd.aurora.quality-control+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )?;
    let receipt = QualityControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        dataset_id: canonical_request.dataset_id,
        modality: canonical_request.modality,
        source_digest: canonical_request.source_digest,
        request_digest,
        metrics: canonical_request.metrics,
        conformance_verified: canonical_request.conformance_verified,
        summary,
        semantic_loss,
        artifact,
        raw_data_local: canonical_request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(metrics: Vec<QualityMetric>) -> QualityControlRequest {
        QualityControlRequest {
            dataset_id: "study-1".into(),
            modality: "imaging".into(),
            source_digest: ContentHash::of_bytes(b"study-1"),
            metrics,
            conformance_verified: true,
            raw_data_local: true,
        }
    }

    fn metric(id: &str, value: Option<f64>, required: bool) -> QualityMetric {
        QualityMetric {
            metric_id: id.into(),
            value,
            threshold: 0.9,
            direction: MetricDirection::AtLeast,
            required,
        }
    }

    #[test]
    fn missing_required_metric_is_unknown_and_not_a_pass() {
        let receipt =
            evaluate_quality_control(&request(vec![metric("focus", None, true)])).unwrap();
        assert_eq!(receipt.summary.disposition, QualityDisposition::Unknown);
        assert_eq!(receipt.summary.unknown, 1);
    }

    #[test]
    fn optional_metric_failure_is_a_warning() {
        let receipt =
            evaluate_quality_control(&request(vec![metric("focus", Some(0.4), false)])).unwrap();
        assert_eq!(
            receipt.summary.disposition,
            QualityDisposition::PassWithWarnings
        );
        assert_eq!(receipt.summary.warnings, 1);
    }

    #[test]
    fn unverified_or_nonlocal_data_is_blocked() {
        let mut input = request(vec![metric("focus", Some(0.95), true)]);
        input.conformance_verified = false;
        let receipt = evaluate_quality_control(&input).unwrap();
        assert_eq!(receipt.summary.disposition, QualityDisposition::Blocked);
    }

    #[test]
    fn identical_requests_have_identical_receipt_digests() {
        let input = request(vec![metric("focus", Some(0.95), true)]);
        let left = evaluate_quality_control(&input).unwrap();
        let right = evaluate_quality_control(&input).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        manifest().validate().unwrap();
    }

    #[test]
    fn metric_input_order_is_canonicalized() {
        let ordered = request(vec![
            metric("focus", Some(0.95), true),
            metric("signal", Some(0.95), true),
        ]);
        let mut reversed = ordered.clone();
        reversed.metrics.reverse();
        let left = evaluate_quality_control(&ordered).unwrap();
        let right = evaluate_quality_control(&reversed).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.request_digest, right.request_digest);
    }

    #[test]
    fn retained_metric_tampering_is_rejected() {
        let mut receipt =
            evaluate_quality_control(&request(vec![metric("focus", Some(0.95), true)])).unwrap();
        receipt.metrics[0].value = Some(0.1);
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn quality_artifact_payload_tampering_is_rejected() {
        let mut receipt =
            evaluate_quality_control(&request(vec![metric("focus", Some(0.95), true)])).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }
}
