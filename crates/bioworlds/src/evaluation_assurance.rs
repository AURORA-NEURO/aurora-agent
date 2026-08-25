//! Federated continual evaluation and observability assurance harness.
//!
//! Atlas feature: `AFA-bioworlds-P23-F28`.
//! This harness turns distributed benchmark observations into an auditable
//! evaluation receipt. It never upgrades incomplete evidence, exports raw
//! measurements, or hides null/negative outcomes.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioworlds-P23-F28";
pub const CONTRACT_VERSION: &str = "bioworlds-federated-evaluation-observability-assurance/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricOutcome {
    Positive,
    Null,
    Negative,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationObservation {
    pub observation_id: String,
    pub site_id: String,
    pub metric_id: String,
    pub scope: String,
    pub value_milli: i64,
    pub uncertainty_milli: u64,
    pub outcome: MetricOutcome,
    pub state: ObservationState,
    pub baseline_digest: Option<ContentHash>,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub comparable: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub capability_id: String,
    pub benchmark_id: String,
    pub scope: String,
    pub required_metric_ids: Vec<String>,
    pub minimum_independent_sites: u16,
    pub observations: Vec<EvaluationObservation>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_allow: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationDisposition {
    Passed,
    Conditional,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationSummary {
    pub summary_id: String,
    pub disposition: EvaluationDisposition,
    pub observation_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub metric_order: Vec<String>,
    pub site_order: Vec<String>,
    pub baseline_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub positive_count: u32,
    pub null_count: u32,
    pub negative_count: u32,
    pub inconclusive_count: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub summary_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub capability_id: String,
    pub benchmark_id: String,
    pub disposition: EvaluationDisposition,
    pub summary: EvaluationSummary,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvaluationAssuranceError {
    #[error("invalid evaluation assurance request: {0}")]
    Invalid(String),
    #[error("evaluation assurance serialization failed: {0}")]
    Serialization(String),
}

impl EvaluationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), EvaluationAssuranceError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.benchmark_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.summary.boundary != PRECLINICAL_BOUNDARY
            || self.summary.summary_id.trim().is_empty()
            || (self.summary.admitted_order.is_empty()
                && self.summary.blocked_order.is_empty()
                && self.summary.omissions.is_empty()
                && self.summary.uncertainty.is_empty()
                && self.summary.negative_evidence.is_empty())
        {
            return Err(EvaluationAssuranceError::Invalid(
                "evaluation assurance identity, summary, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.summary.observation_order,
            &self.summary.admitted_order,
            &self.summary.blocked_order,
            &self.summary.metric_order,
            &self.summary.site_order,
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
                return Err(EvaluationAssuranceError::Invalid(
                    "evaluation assurance ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.summary.baseline_order,
            &self.summary.artifact_order,
            &self.summary.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvaluationAssuranceError::Invalid(
                    "evaluation assurance digest ordering is not canonical".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, EvaluationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))
    }
}

pub fn assure_evaluation_observability(
    request: &EvaluationAssuranceRequest,
) -> Result<EvaluationAssuranceReceipt, EvaluationAssuranceError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let required_metrics = request
        .required_metric_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observation_order = BTreeSet::new();
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut metrics = BTreeSet::new();
    let mut sites = BTreeSet::new();
    let mut baselines = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut positive_count = 0_u32;
    let mut null_count = 0_u32;
    let mut negative_count = 0_u32;
    let mut inconclusive_count = 0_u32;
    let mut spent = 0_u64;
    for observation in &observations {
        observation_order.insert(observation.observation_id.clone());
        let cost = observation.observation_id.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = observation.state == ObservationState::Supported
            && observation.comparable
            && observation.baseline_digest.is_some()
            && observation.scope == request.scope
            && observation.omissions.is_empty()
            && observation.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.protected_closure
            && request.federation_allow
            && request.signed_approval
            && request.raw_data_local
            && complete
            && budget_ok;
        match observation.outcome {
            MetricOutcome::Positive => positive_count = positive_count.saturating_add(1),
            MetricOutcome::Null => null_count = null_count.saturating_add(1),
            MetricOutcome::Negative => negative_count = negative_count.saturating_add(1),
            MetricOutcome::Inconclusive => {
                inconclusive_count = inconclusive_count.saturating_add(1)
            }
        }
        if gate {
            spent = spent.saturating_add(cost);
            admitted.insert(observation.observation_id.clone());
            metrics.insert(observation.metric_id.clone());
            sites.insert(observation.site_id.clone());
            if let Some(baseline) = &observation.baseline_digest {
                baselines.insert(baseline.clone());
            }
            artifacts.insert(observation.artifact_digest.clone());
            provenance.insert(observation.provenance_digest.clone());
            if matches!(
                observation.outcome,
                MetricOutcome::Null | MetricOutcome::Negative
            ) {
                negative.insert(
                    format!(
                        "observation:{}:outcome-{:?}-retained",
                        observation.observation_id, observation.outcome
                    )
                    .to_ascii_lowercase(),
                );
            }
        } else {
            blocked.insert(observation.observation_id.clone());
            if observation.state != ObservationState::Supported {
                negative.insert(
                    format!(
                        "observation:{}:state-{:?}-not-admitted",
                        observation.observation_id, observation.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if !observation.comparable {
                omissions.insert(format!(
                    "observation:{}:comparability-not-established",
                    observation.observation_id
                ));
            }
            if observation.baseline_digest.is_none() {
                omissions.insert(format!(
                    "observation:{}:baseline-missing",
                    observation.observation_id
                ));
            }
            if observation.scope != request.scope {
                omissions.insert(format!(
                    "observation:{}:scope-mismatch",
                    observation.observation_id
                ));
            }
            if !observation.omissions.is_empty() || !observation.uncertainty.is_empty() {
                uncertainty.insert(format!(
                    "observation:{}:protected-closure-or-evidence-incomplete",
                    observation.observation_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "observation:{}:budget-ceiling-exceeded",
                    observation.observation_id
                ));
            }
        }
    }
    for metric in required_metrics {
        if !metrics.contains(&metric) {
            omissions.insert(format!("metric:{metric}:required-but-not-admitted"));
        }
    }
    if sites.len() < request.minimum_independent_sites as usize {
        uncertainty.insert(format!(
            "site-floor:{}-of-{}-independent-sites",
            sites.len(),
            request.minimum_independent_sites
        ));
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow || !request.federation_allow {
        EvaluationDisposition::Blocked
    } else if !request.protected_closure || admitted_order.is_empty() {
        EvaluationDisposition::Unknown
    } else if blocked_order.is_empty() && omissions.is_empty() && uncertainty.is_empty() {
        EvaluationDisposition::Passed
    } else {
        EvaluationDisposition::Conditional
    };
    let mut checks = vec![
        "observation, metric, site, and digest ordering is canonical".into(),
        "comparability, baseline, provenance, policy, federation, approval, locality, budget, and site-floor gates are explicit".into(),
        "null, negative, contradicted, unknown, unmeasured, omitted, and inconclusive outcomes remain visible".into(),
        "federation exchanges only evaluation manifests and content digests".into(),
    ];
    checks.sort();
    let observation_order = observation_order.into_iter().collect::<Vec<_>>();
    let metric_order = metrics.into_iter().collect::<Vec<_>>();
    let site_order = sites.into_iter().collect::<Vec<_>>();
    let baseline_order = baselines.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = observation_order
        .iter()
        .map(|observation_id| format!("exchange:evaluation-manifest-digest-only:{observation_id}"))
        .collect::<Vec<_>>();
    let summary_id = format!("evaluation-summary:{}", request.request_id);
    let summary_payload = json!({
        "summary_id": summary_id,
        "disposition": disposition,
        "observation_order": observation_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "metric_order": metric_order,
        "site_order": site_order,
        "baseline_order": baseline_order,
        "artifact_order": artifact_order,
        "provenance_order": provenance_order,
        "positive_count": positive_count,
        "null_count": null_count,
        "negative_count": negative_count,
        "inconclusive_count": inconclusive_count,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let summary_digest = ContentHash::of_value(&summary_payload)
        .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))?;
    let summary = EvaluationSummary {
        summary_id,
        disposition,
        observation_order,
        admitted_order,
        blocked_order,
        metric_order,
        site_order,
        baseline_order,
        artifact_order,
        provenance_order,
        positive_count,
        null_count,
        negative_count,
        inconclusive_count,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        replay_identity: request.replay_identity.clone(),
        summary_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = EvaluationAssuranceReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        capability_id: request.capability_id.clone(),
        benchmark_id: request.benchmark_id.clone(),
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

fn validate_request(request: &EvaluationAssuranceRequest) -> Result<(), EvaluationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.capability_id.trim().is_empty()
        || request.benchmark_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_metric_ids.is_empty()
        || request.minimum_independent_sites == 0
        || request.observations.is_empty()
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .required_metric_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(EvaluationAssuranceError::Invalid(
            "evaluation assurance identity, scope, metrics, site floor, observations, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || observation.site_id.trim().is_empty()
            || observation.metric_id.trim().is_empty()
            || observation.scope.trim().is_empty()
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
            return Err(EvaluationAssuranceError::Invalid(format!(
                "evaluation observation {} is invalid or duplicated",
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
        site: &str,
        metric: &str,
        outcome: MetricOutcome,
        state: ObservationState,
    ) -> EvaluationObservation {
        EvaluationObservation {
            observation_id: id.into(),
            site_id: site.into(),
            metric_id: metric.into(),
            scope: "organoid:neural".into(),
            value_milli: 120,
            uncertainty_milli: 15,
            outcome,
            state,
            baseline_digest: Some(hash("baseline")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            comparable: true,
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(observations: Vec<EvaluationObservation>) -> EvaluationAssuranceRequest {
        EvaluationAssuranceRequest {
            request_id: "evaluation:assurance".into(),
            workflow_id: "workflow:evaluation".into(),
            capability_id: "capability:mechanism".into(),
            benchmark_id: "benchmark:organoid".into(),
            scope: "organoid:neural".into(),
            required_metric_ids: vec!["metric:effect".into(), "metric:robustness".into()],
            minimum_independent_sites: 2,
            observations,
            replay_identity: hash("replay"),
            budget: 100,
            policy_allow: true,
            protected_closure: true,
            federation_allow: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn passes_with_complete_multi_site_metrics() {
        let receipt = assure_evaluation_observability(&request(vec![
            observation(
                "observation:a",
                "site:a",
                "metric:effect",
                MetricOutcome::Positive,
                ObservationState::Supported,
            ),
            observation(
                "observation:b",
                "site:b",
                "metric:robustness",
                MetricOutcome::Null,
                ObservationState::Supported,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, EvaluationDisposition::Passed);
        assert_eq!(receipt.summary.null_count, 1);
        assert!(!receipt.negative_evidence.is_empty());
    }

    #[test]
    fn null_result_is_retained_not_suppressed() {
        let receipt = assure_evaluation_observability(&request(vec![
            observation(
                "observation:a",
                "site:a",
                "metric:effect",
                MetricOutcome::Null,
                ObservationState::Supported,
            ),
            observation(
                "observation:b",
                "site:b",
                "metric:robustness",
                MetricOutcome::Negative,
                ObservationState::Supported,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.summary.null_count, 1);
        assert_eq!(receipt.summary.negative_count, 1);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("null")));
    }

    #[test]
    fn insufficient_site_floor_is_conditional() {
        let receipt = assure_evaluation_observability(&request(vec![
            observation(
                "observation:a",
                "site:a",
                "metric:effect",
                MetricOutcome::Positive,
                ObservationState::Supported,
            ),
            observation(
                "observation:b",
                "site:a",
                "metric:robustness",
                MetricOutcome::Positive,
                ObservationState::Supported,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, EvaluationDisposition::Conditional);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("site-floor")));
    }

    #[test]
    fn contradiction_is_blocked_with_negative_evidence() {
        let receipt = assure_evaluation_observability(&request(vec![
            observation(
                "observation:a",
                "site:a",
                "metric:effect",
                MetricOutcome::Positive,
                ObservationState::Contradicted,
            ),
            observation(
                "observation:b",
                "site:b",
                "metric:robustness",
                MetricOutcome::Positive,
                ObservationState::Supported,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, EvaluationDisposition::Conditional);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
    }

    #[test]
    fn duplicate_observations_are_rejected() {
        let result = assure_evaluation_observability(&request(vec![
            observation(
                "observation:a",
                "site:a",
                "metric:effect",
                MetricOutcome::Positive,
                ObservationState::Supported,
            ),
            observation(
                "observation:a",
                "site:b",
                "metric:robustness",
                MetricOutcome::Positive,
                ObservationState::Supported,
            ),
        ]));
        assert!(result.is_err());
    }
}
