//! Prospective high-throughput quality-control research workbench.
//!
//! Atlas feature: `AFA-biolang-P07-F19`.
//! The workbench evaluates a deterministic stream of typed assay observations,
//! separates release, warning, quarantine, and unresolved states, and emits a
//! local quality manifest. Raw experimental data never leaves the institution.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-biolang-P07-F19";
pub const CONTRACT_VERSION: &str = "biolang-prospective-quality-workbench/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricDirection {
    AtLeast,
    AtMost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityDisposition {
    Released,
    Conditional,
    Quarantined,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityObservation {
    pub observation_id: String,
    pub batch_id: String,
    pub sample_id: String,
    pub modality: String,
    pub metric_id: String,
    pub value_milli: Option<i64>,
    pub threshold_milli: i64,
    pub direction: MetricDirection,
    pub required: bool,
    pub state: QualityState,
    pub baseline_digest: Option<ContentHash>,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityWorkbenchRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub study_id: String,
    pub scope: String,
    pub required_metric_ids: Vec<String>,
    pub minimum_release_fraction_milli: u16,
    pub observations: Vec<QualityObservation>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityWorkbenchSummary {
    pub summary_id: String,
    pub disposition: QualityDisposition,
    pub observation_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub warning_order: Vec<String>,
    pub quarantined_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub batch_order: Vec<String>,
    pub sample_order: Vec<String>,
    pub metric_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub passed_count: u32,
    pub warning_count: u32,
    pub quarantined_count: u32,
    pub unknown_count: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub summary_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub study_id: String,
    pub disposition: QualityDisposition,
    pub summary: QualityWorkbenchSummary,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityWorkbenchError {
    #[error("invalid quality workbench request: {0}")]
    Invalid(String),
    #[error("quality workbench serialization failed: {0}")]
    Serialization(String),
}

impl QualityWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), QualityWorkbenchError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.summary.boundary != PRECLINICAL_BOUNDARY
            || self.summary.summary_id.trim().is_empty()
            || (self.summary.qualified_order.is_empty()
                && self.summary.warning_order.is_empty()
                && self.summary.quarantined_order.is_empty()
                && self.summary.unknown_order.is_empty()
                && self.summary.omissions.is_empty()
                && self.summary.uncertainty.is_empty()
                && self.summary.negative_evidence.is_empty())
        {
            return Err(QualityWorkbenchError::Invalid(
                "quality workbench identity, summary, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.summary.observation_order,
            &self.summary.qualified_order,
            &self.summary.warning_order,
            &self.summary.quarantined_order,
            &self.summary.unknown_order,
            &self.summary.batch_order,
            &self.summary.sample_order,
            &self.summary.metric_order,
            &self.summary.omissions,
            &self.summary.uncertainty,
            &self.summary.negative_evidence,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(QualityWorkbenchError::Invalid(
                    "quality workbench ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.summary.artifact_order, &self.summary.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(QualityWorkbenchError::Invalid(
                    "quality workbench digest ordering is not canonical".into(),
                ));
            }
        }
        let classified_ids = self
            .summary
            .qualified_order
            .iter()
            .chain(self.summary.warning_order.iter())
            .chain(self.summary.quarantined_order.iter())
            .chain(self.summary.unknown_order.iter())
            .collect::<BTreeSet<_>>();
        let observation_ids = self
            .summary
            .observation_order
            .iter()
            .collect::<BTreeSet<_>>();
        let counts_match = u64::from(self.summary.passed_count)
            == u64::try_from(self.summary.qualified_order.len()).unwrap_or(u64::MAX)
            && u64::from(self.summary.warning_count)
                == u64::try_from(self.summary.warning_order.len()).unwrap_or(u64::MAX)
            && u64::from(self.summary.quarantined_count)
                == u64::try_from(self.summary.quarantined_order.len()).unwrap_or(u64::MAX)
            && u64::from(self.summary.unknown_count)
                == u64::try_from(self.summary.unknown_order.len()).unwrap_or(u64::MAX);
        if classified_ids != observation_ids || !counts_match {
            return Err(QualityWorkbenchError::Invalid(
                "quality workbench observations are not completely and consistently classified"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, QualityWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| QualityWorkbenchError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| QualityWorkbenchError::Serialization(error.to_string()))
    }
}

pub fn operate_quality_workbench(
    request: &QualityWorkbenchRequest,
) -> Result<QualityWorkbenchReceipt, QualityWorkbenchError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let required_metrics = request
        .required_metric_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observation_order = BTreeSet::new();
    let mut qualified = BTreeSet::new();
    let mut warning = BTreeSet::new();
    let mut quarantined = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut batches = BTreeSet::new();
    let mut samples = BTreeSet::new();
    let mut metrics = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut passed_count = 0_u32;
    let mut warning_count = 0_u32;
    let mut quarantined_count = 0_u32;
    let mut unknown_count = 0_u32;
    let mut spent = 0_u64;
    for observation in &observations {
        observation_order.insert(observation.observation_id.clone());
        batches.insert(observation.batch_id.clone());
        samples.insert(observation.sample_id.clone());
        let cost = u64::try_from(observation.observation_id.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let value_passes =
            observation
                .value_milli
                .is_some_and(|value| match observation.direction {
                    MetricDirection::AtLeast => value >= observation.threshold_milli,
                    MetricDirection::AtMost => value <= observation.threshold_milli,
                });
        let complete = observation.state == QualityState::Supported
            && observation.value_milli.is_some()
            && observation.baseline_digest.is_some()
            && observation.omissions.is_empty()
            && observation.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && complete
            && budget_ok;
        if gate && value_passes {
            spent = spent.saturating_add(cost);
            passed_count = passed_count.saturating_add(1);
            qualified.insert(observation.observation_id.clone());
            metrics.insert(observation.metric_id.clone());
            artifacts.insert(observation.artifact_digest.clone());
            provenance.insert(observation.provenance_digest.clone());
        } else if gate && !observation.required {
            warning_count = warning_count.saturating_add(1);
            warning.insert(observation.observation_id.clone());
            metrics.insert(observation.metric_id.clone());
            artifacts.insert(observation.artifact_digest.clone());
            provenance.insert(observation.provenance_digest.clone());
            uncertainty.insert(format!(
                "observation:{}:optional-metric-below-threshold",
                observation.observation_id
            ));
        } else {
            if !request.policy_allow
                || !request.protected_closure
                || !request.signed_approval
                || !request.raw_data_local
                || observation.value_milli.is_none()
                || matches!(
                    observation.state,
                    QualityState::Unknown | QualityState::Unmeasured
                )
            {
                unknown_count = unknown_count.saturating_add(1);
                unknown.insert(observation.observation_id.clone());
            } else {
                quarantined_count = quarantined_count.saturating_add(1);
                quarantined.insert(observation.observation_id.clone());
            }
            if observation.state != QualityState::Supported {
                negative.insert(
                    format!(
                        "observation:{}:state-{:?}-not-qualified",
                        observation.observation_id, observation.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if observation.baseline_digest.is_none() {
                omissions.insert(format!(
                    "observation:{}:baseline-missing",
                    observation.observation_id
                ));
            }
            if !observation.omissions.is_empty() || !observation.uncertainty.is_empty() {
                uncertainty.insert(format!(
                    "observation:{}:protected-closure-incomplete",
                    observation.observation_id
                ));
            }
            if observation.value_milli.is_some() && !value_passes {
                if observation.required {
                    omissions.insert(format!(
                        "observation:{}:required-threshold-failed",
                        observation.observation_id
                    ));
                } else {
                    uncertainty.insert(format!(
                        "observation:{}:optional-threshold-failed",
                        observation.observation_id
                    ));
                }
            }
            if !budget_ok {
                omissions.insert(format!(
                    "observation:{}:budget-ceiling-exceeded",
                    observation.observation_id
                ));
            }
        }
        if matches!(observation.state, QualityState::Contradicted) {
            negative.insert(format!(
                "observation:{}:contradicted-quality-evidence",
                observation.observation_id
            ));
        }
    }
    for metric in required_metrics {
        if !metrics.contains(&metric) {
            omissions.insert(format!("metric:{metric}:required-but-not-qualified"));
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    if !request.raw_data_local {
        omissions.insert("request:raw-data-locality-required".into());
    }
    let passed_total = u64::try_from(observations.len()).unwrap_or(u64::MAX).max(1);
    let release_fraction = (passed_count as u64 * 1000) / passed_total;
    if release_fraction < u64::from(request.minimum_release_fraction_milli) {
        uncertainty.insert(format!(
            "release-fraction:{}-below-required-{}",
            release_fraction, request.minimum_release_fraction_milli
        ));
    }
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let warning_order = warning.into_iter().collect::<Vec<_>>();
    let quarantined_order = quarantined.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow || !request.raw_data_local {
        QualityDisposition::Blocked
    } else if !request.protected_closure || qualified_order.is_empty() && warning_order.is_empty() {
        QualityDisposition::Unknown
    } else if !quarantined_order.is_empty()
        || !unknown_order.is_empty()
        || !omissions.is_empty()
        || release_fraction < u64::from(request.minimum_release_fraction_milli)
        || !warning_order.is_empty()
    {
        QualityDisposition::Conditional
    } else {
        QualityDisposition::Released
    };
    let mut checks = vec![
        "observation, batch, sample, metric, and digest ordering is canonical".into(),
        "threshold, baseline, evidence, policy, protected-closure, approval, locality, budget, and release-fraction gates are explicit".into(),
        "unknown, unmeasured, contradicted, quarantined, warning, and negative quality states remain visible".into(),
        "raw experimental data remains local; only typed quality manifests are emitted".into(),
    ];
    checks.sort();
    let observation_order = observation_order.into_iter().collect::<Vec<_>>();
    let batch_order = batches.into_iter().collect::<Vec<_>>();
    let sample_order = samples.into_iter().collect::<Vec<_>>();
    let metric_order = metrics.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = batch_order
        .iter()
        .map(|batch_id| format!("write:local-quality-manifest:{batch_id}"))
        .collect::<Vec<_>>();
    let summary_id = format!("quality-summary:{}", request.request_id);
    let summary_payload = json!({
        "summary_id": summary_id,
        "disposition": disposition,
        "observation_order": observation_order,
        "qualified_order": qualified_order,
        "warning_order": warning_order,
        "quarantined_order": quarantined_order,
        "unknown_order": unknown_order,
        "batch_order": batch_order,
        "sample_order": sample_order,
        "metric_order": metric_order,
        "artifact_order": artifact_order,
        "provenance_order": provenance_order,
        "passed_count": passed_count,
        "warning_count": warning_count,
        "quarantined_count": quarantined_count,
        "unknown_count": unknown_count,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let summary_digest = ContentHash::of_value(&summary_payload)
        .map_err(|error| QualityWorkbenchError::Serialization(error.to_string()))?;
    let summary = QualityWorkbenchSummary {
        summary_id,
        disposition,
        observation_order,
        qualified_order,
        warning_order,
        quarantined_order,
        unknown_order,
        batch_order,
        sample_order,
        metric_order,
        artifact_order,
        provenance_order,
        passed_count,
        warning_count,
        quarantined_count,
        unknown_count,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        replay_identity: request.replay_identity.clone(),
        summary_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = QualityWorkbenchReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        study_id: request.study_id.clone(),
        disposition,
        summary,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &QualityWorkbenchRequest) -> Result<(), QualityWorkbenchError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_metric_ids.is_empty()
        || request.observations.is_empty()
        || request.budget == 0
        || request.minimum_release_fraction_milli > 1000
        || u64::try_from(request.observations.len())
            .map_or(true, |count| count > u64::from(u32::MAX))
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .required_metric_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(QualityWorkbenchError::Invalid(
            "quality workbench identity, scope, metrics, observations, release threshold, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || observation.batch_id.trim().is_empty()
            || observation.sample_id.trim().is_empty()
            || observation.modality.trim().is_empty()
            || observation.metric_id.trim().is_empty()
            || !ids.insert(observation.observation_id.clone())
            || observation.boundary != PRECLINICAL_BOUNDARY
            || observation
                .omissions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || observation
                .uncertainty
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || observation
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(QualityWorkbenchError::Invalid(format!(
                "quality observation {} is invalid or duplicated",
                observation.observation_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn observation(
        id: &str,
        metric: &str,
        value: Option<i64>,
        required: bool,
        state: QualityState,
    ) -> QualityObservation {
        QualityObservation {
            observation_id: id.into(),
            batch_id: "batch:a".into(),
            sample_id: format!("sample:{id}"),
            modality: "imaging".into(),
            metric_id: metric.into(),
            value_milli: value,
            threshold_milli: 900,
            direction: MetricDirection::AtLeast,
            required,
            state,
            baseline_digest: Some(hash("baseline")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(observations: Vec<QualityObservation>) -> QualityWorkbenchRequest {
        QualityWorkbenchRequest {
            request_id: "quality:workbench".into(),
            workflow_id: "workflow:quality".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            required_metric_ids: vec!["metric:focus".into(), "metric:signal".into()],
            minimum_release_fraction_milli: 500,
            observations,
            replay_identity: hash("replay"),
            budget: 100,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn releases_complete_high_throughput_quality_batch() {
        let receipt = operate_quality_workbench(&request(vec![
            observation(
                "observation:a",
                "metric:focus",
                Some(950),
                true,
                QualityState::Supported,
            ),
            observation(
                "observation:b",
                "metric:signal",
                Some(920),
                true,
                QualityState::Supported,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, QualityDisposition::Released);
        assert_eq!(receipt.summary.passed_count, 2);
        assert!(receipt.effect_receipts[0].starts_with("write:local-quality-manifest:"));
    }

    #[test]
    fn missing_value_is_unknown_not_a_pass() {
        let receipt = operate_quality_workbench(&request(vec![
            observation(
                "observation:a",
                "metric:focus",
                None,
                true,
                QualityState::Unmeasured,
            ),
            observation(
                "observation:b",
                "metric:signal",
                Some(920),
                true,
                QualityState::Supported,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, QualityDisposition::Conditional);
        assert_eq!(receipt.summary.unknown_count, 1);
    }

    #[test]
    fn optional_threshold_failure_is_warning() {
        let receipt = operate_quality_workbench(&request(vec![
            observation(
                "observation:a",
                "metric:focus",
                Some(800),
                false,
                QualityState::Supported,
            ),
            observation(
                "observation:b",
                "metric:signal",
                Some(920),
                true,
                QualityState::Supported,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.summary.warning_count, 1);
        assert_eq!(receipt.disposition, QualityDisposition::Conditional);
    }

    #[test]
    fn contradiction_is_quarantined_with_negative_evidence() {
        let receipt = operate_quality_workbench(&request(vec![
            observation(
                "observation:a",
                "metric:focus",
                Some(950),
                true,
                QualityState::Contradicted,
            ),
            observation(
                "observation:b",
                "metric:signal",
                Some(920),
                true,
                QualityState::Supported,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.summary.quarantined_count, 1);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
    }

    #[test]
    fn duplicate_observations_are_rejected() {
        let result = operate_quality_workbench(&request(vec![
            observation(
                "observation:a",
                "metric:focus",
                Some(950),
                true,
                QualityState::Supported,
            ),
            observation(
                "observation:a",
                "metric:signal",
                Some(920),
                true,
                QualityState::Supported,
            ),
        ]));
        assert!(result.is_err());
    }

    #[test]
    fn non_local_raw_data_blocks_release_without_emitting_raw_data() {
        let mut request = request(vec![observation(
            "observation:a",
            "metric:focus",
            Some(950),
            true,
            QualityState::Supported,
        )]);
        request.raw_data_local = false;
        let receipt = operate_quality_workbench(&request).unwrap();
        assert_eq!(receipt.disposition, QualityDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "request:raw-data-locality-required"));
    }
}
