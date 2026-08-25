//! Multimodal multi-study quality-control assurance.
//!
//! Atlas feature: `AFA-bioevalx-P07-F26`.
//! This is a witness-bearing quality boundary: it compares only explicitly
//! comparable preclinical research objects, retains failed and unmeasured
//! quality evidence, and emits a typed verdict without moving raw data.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioevalx-P07-F26";
pub const CONTRACT_VERSION: &str = "bioevalx-multimodal-quality-assurance/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityState {
    Pass,
    Fail,
    Unknown,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityMetric {
    pub metric_id: String,
    pub modality: String,
    pub measurement_digest: ContentHash,
    pub threshold_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub state: QualityState,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObject {
    pub object_id: String,
    pub study_id: String,
    pub scope: String,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub comparability_digest: ContentHash,
    pub metrics: Vec<QualityMetric>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub objects: Vec<ResearchObject>,
    pub minimum_studies: u32,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityVerdict {
    pub verdict_id: String,
    pub disposition: QualityDisposition,
    pub study_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub comparability_digest: Option<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub witness_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub verdict_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub disposition: QualityDisposition,
    pub verdict: QualityVerdict,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityAssuranceError {
    #[error("invalid quality assurance request: {0}")]
    Invalid(String),
    #[error("quality assurance serialization failed: {0}")]
    Serialization(String),
}

impl QualityAssuranceReceipt {
    pub fn validate(&self) -> Result<(), QualityAssuranceError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.verdict.boundary != PRECLINICAL_BOUNDARY
            || (self.verdict.qualified_order.is_empty()
                && self.verdict.blocked_order.is_empty()
                && self.verdict.omissions.is_empty()
                && self.verdict.uncertainty.is_empty()
                && self.verdict.negative_evidence.is_empty())
        {
            return Err(QualityAssuranceError::Invalid(
                "quality identity, witness verdict, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.verdict.study_order,
            &self.verdict.qualified_order,
            &self.verdict.blocked_order,
            &self.verdict.witness_order,
            &self.verdict.omissions,
            &self.verdict.uncertainty,
            &self.verdict.negative_evidence,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(QualityAssuranceError::Invalid(
                    "quality assurance ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.verdict.artifact_order, &self.verdict.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(QualityAssuranceError::Invalid(
                    "quality assurance digest ordering is not canonical".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, QualityAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| QualityAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| QualityAssuranceError::Serialization(error.to_string()))
    }
}

pub fn assure_quality(
    request: &QualityAssuranceRequest,
) -> Result<QualityAssuranceReceipt, QualityAssuranceError> {
    validate_request(request)?;
    let mut objects = request.objects.clone();
    objects.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let mut studies = BTreeSet::new();
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut witnesses = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let comparability = objects
        .first()
        .map(|object| object.comparability_digest.clone());
    let mut spent = 0_u64;
    for object in &objects {
        studies.insert(object.study_id.clone());
        let cost = object.metrics.len() as u64 + object.artifact_order.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let comparable = comparability
            .as_ref()
            .map(|digest| digest == &object.comparability_digest)
            .unwrap_or(false);
        let metrics_pass = !object.metrics.is_empty()
            && object
                .metrics
                .iter()
                .all(|metric| metric.state == QualityState::Pass);
        let complete = !object.artifact_order.is_empty()
            && !object.provenance_order.is_empty()
            && object.omissions.is_empty()
            && object.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && comparable
            && metrics_pass
            && complete
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            qualified.insert(object.study_id.clone());
            artifacts.extend(object.artifact_order.iter().cloned());
            provenance.extend(object.provenance_order.iter().cloned());
        } else {
            blocked.insert(object.study_id.clone());
            if !comparable {
                witnesses.insert(format!("study:{}:comparability-mismatch", object.study_id));
                negative.insert(format!(
                    "study:{}:cross-study-comparability-conflict",
                    object.study_id
                ));
            }
            if !metrics_pass {
                witnesses.insert(format!("study:{}:quality-metric-not-pass", object.study_id));
                negative.insert(format!(
                    "study:{}:failed-or-unmeasured-quality",
                    object.study_id
                ));
            }
            if !complete {
                omissions.insert(format!(
                    "study:{}:protected-quality-closure-incomplete",
                    object.study_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!("study:{}:budget-ceiling-exceeded", object.study_id));
            }
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    let study_order = studies.into_iter().collect::<Vec<_>>();
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow || !request.signed_approval {
        QualityDisposition::Blocked
    } else if !request.protected_closure {
        QualityDisposition::Unknown
    } else if qualified_order.len() < request.minimum_studies as usize {
        QualityDisposition::Partial
    } else if blocked_order.is_empty() {
        QualityDisposition::Qualified
    } else {
        QualityDisposition::Partial
    };
    let mut checks = vec![
        "canonical study, artifact, provenance, witness, omission, and effect ordering".into(),
        "cross-study comparability and modality quality metrics are explicit gates".into(),
        "unknown, failed, unmeasured, and protected-closure gaps cannot become a pass".into(),
        "raw research-object payloads remain local and replay identity binds the verdict".into(),
    ];
    checks.sort();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let witness_order = witnesses.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let mut effect_receipts = if !qualified_order.is_empty() {
        qualified_order
            .iter()
            .map(|study| format!("exchange:permitted-quality-manifest:{study}"))
            .collect::<Vec<_>>()
    } else {
        vec![format!("block:quality-assurance:{disposition:?}").to_ascii_lowercase()]
    };
    effect_receipts.sort();
    let verdict_id = format!("quality-verdict:{}", request.request_id);
    let verdict_payload = json!({
        "verdict_id": verdict_id,
        "disposition": disposition,
        "study_order": study_order,
        "qualified_order": qualified_order,
        "blocked_order": blocked_order,
        "comparability_digest": comparability,
        "artifact_order": artifact_order,
        "provenance_order": provenance_order,
        "witness_order": witness_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let verdict_digest = ContentHash::of_value(&verdict_payload)
        .map_err(|error| QualityAssuranceError::Serialization(error.to_string()))?;
    let verdict = QualityVerdict {
        verdict_id,
        disposition,
        study_order,
        qualified_order,
        blocked_order,
        comparability_digest: comparability,
        artifact_order,
        provenance_order,
        witness_order,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        replay_identity: request.replay_identity.clone(),
        verdict_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = QualityAssuranceReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        disposition,
        verdict,
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

fn validate_request(request: &QualityAssuranceRequest) -> Result<(), QualityAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.objects.len() < 2
        || request.minimum_studies == 0
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(QualityAssuranceError::Invalid(
            "quality identity, multi-study set, minimum floor, budget, or boundary is incomplete"
                .into(),
        ));
    }
    let mut studies = BTreeSet::new();
    for object in &request.objects {
        if object.object_id.trim().is_empty()
            || object.study_id.trim().is_empty()
            || !studies.insert(object.study_id.clone())
            || object.scope.trim().is_empty()
            || object.modality_order.is_empty()
            || object.boundary != PRECLINICAL_BOUNDARY
            || object
                .modality_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(QualityAssuranceError::Invalid(format!(
                "research object {} is invalid or duplicated",
                object.object_id
            )));
        }
        for metric in &object.metrics {
            if metric.metric_id.trim().is_empty()
                || metric.modality.trim().is_empty()
                || metric.boundary != PRECLINICAL_BOUNDARY
            {
                return Err(QualityAssuranceError::Invalid(format!(
                    "quality metric {} is invalid",
                    metric.metric_id
                )));
            }
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

    fn object(study: &str, state: QualityState, comparability: &str) -> ResearchObject {
        ResearchObject {
            object_id: format!("object:{study}"),
            study_id: study.into(),
            scope: "organoid:neural".into(),
            modality_order: vec!["imaging".into(), "omics".into()],
            artifact_order: vec![hash(&format!("artifact:{study}"))],
            provenance_order: vec![hash(&format!("provenance:{study}"))],
            comparability_digest: hash(comparability),
            metrics: vec![QualityMetric {
                metric_id: format!("metric:{study}"),
                modality: "imaging".into(),
                measurement_digest: hash(&format!("measurement:{study}")),
                threshold_digest: hash("threshold"),
                provenance_digest: hash(&format!("metric-provenance:{study}")),
                state,
                boundary: PRECLINICAL_BOUNDARY.into(),
            }],
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(objects: Vec<ResearchObject>) -> QualityAssuranceRequest {
        QualityAssuranceRequest {
            request_id: "quality:assurance".into(),
            workflow_id: "workflow:multi-study-qc".into(),
            objects,
            minimum_studies: 2,
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
    fn qualifies_comparable_passing_studies() {
        let receipt = assure_quality(&request(vec![
            object("study:a", QualityState::Pass, "comparability"),
            object("study:b", QualityState::Pass, "comparability"),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, QualityDisposition::Qualified);
        assert_eq!(receipt.verdict.qualified_order.len(), 2);
        assert_eq!(receipt.digest(), receipt.digest());
    }

    #[test]
    fn failed_quality_retains_witness_and_negative_evidence() {
        let receipt = assure_quality(&request(vec![
            object("study:a", QualityState::Pass, "comparability"),
            object("study:b", QualityState::Fail, "comparability"),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, QualityDisposition::Partial);
        assert!(!receipt.verdict.witness_order.is_empty());
        assert!(!receipt.negative_evidence.is_empty());
    }

    #[test]
    fn comparability_mismatch_is_blocked_for_that_study() {
        let receipt = assure_quality(&request(vec![
            object("study:a", QualityState::Pass, "comparability-a"),
            object("study:b", QualityState::Pass, "comparability-b"),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, QualityDisposition::Partial);
        assert!(receipt
            .verdict
            .witness_order
            .iter()
            .any(|item| item.contains("comparability-mismatch")));
    }

    #[test]
    fn protected_closure_gap_is_unknown() {
        let mut input = request(vec![
            object("study:a", QualityState::Pass, "comparability"),
            object("study:b", QualityState::Pass, "comparability"),
        ]);
        input.protected_closure = false;
        let receipt = assure_quality(&input).unwrap();
        assert_eq!(receipt.disposition, QualityDisposition::Unknown);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("protected-closure")));
    }

    #[test]
    fn single_study_request_is_rejected() {
        let result = assure_quality(&request(vec![object(
            "study:a",
            QualityState::Pass,
            "comparability",
        )]));
        assert!(result.is_err());
    }
}
