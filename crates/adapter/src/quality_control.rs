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
        let mut ids = BTreeSet::new();
        for metric in &self.metrics {
            if metric.metric_id.trim().is_empty() || !ids.insert(metric.metric_id.clone()) {
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
    pub feature_id: String,
    pub dataset_id: String,
    pub modality: String,
    pub request_digest: ContentHash,
    pub summary: QualityControlSummary,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl QualityControlReceipt {
    pub fn validate(&self) -> Result<(), QualityControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
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
        self.artifact
            .validate_metadata()
            .map_err(QualityControlError::Contract)
    }

    pub fn verify_payload(&self, payload: &Value) -> Result<(), QualityControlError> {
        self.validate()?;
        self.artifact
            .verify_payload(payload)
            .map_err(QualityControlError::Contract)
    }

    pub fn digest(&self) -> Result<ContentHash, QualityControlError> {
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

pub fn evaluate_quality_control(
    request: &QualityControlRequest,
) -> Result<QualityControlReceipt, QualityControlError> {
    request.validate()?;
    let request_value = serde_json::to_value(request)
        .map_err(|error| QualityControlError::Serialization(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| QualityControlError::Serialization(error.to_string()))?;
    let mut passed = 0;
    let mut warnings = 0;
    let mut blocked = 0;
    let mut unknown = 0;
    let mut reasons = Vec::new();
    let metric_rows = request
        .metrics
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
    if !request.conformance_verified {
        blocked += 1;
        reasons.push("adapter conformance is not verified".into());
    }
    if !request.raw_data_local {
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
    let summary = QualityControlSummary {
        disposition,
        passed,
        warnings,
        blocked,
        unknown,
        reasons,
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "dataset_id": request.dataset_id,
        "modality": request.modality,
        "source_digest": request.source_digest,
        "request_digest": request_digest,
        "summary": summary,
        "metrics": metric_rows,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("quality-control:{}", request.dataset_id),
        "application/vnd.aurora.quality-control+json",
        &payload,
        vec![SemanticLoss {
            field: "raw_data".into(),
            reason: "raw experimental data remains institution-local; receipt exports metrics and digests only".into(),
            severity: LossSeverity::Bounded,
        }],
        vec![ProvenanceLink {
            source_id: request.dataset_id.clone(),
            relation: "quality-control-source-digest".into(),
            digest: request.source_digest.clone(),
        }],
    )?;
    let receipt = QualityControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        dataset_id: request.dataset_id.clone(),
        modality: request.modality.clone(),
        request_digest,
        summary,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    receipt.verify_payload(&payload)?;
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
}
