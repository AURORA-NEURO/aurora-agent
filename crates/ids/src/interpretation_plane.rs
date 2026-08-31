//! Deterministic interpretation and federation control for preclinical research.
//!
//! Atlas feature: `AFA-ids-P14-F31`.
//! The plane admits only typed, content-addressed interpretation summaries. Raw
//! imaging or omics bytes remain institution-local; unknown, contradictory, or
//! incompletely protected interpretations cannot be exported as conclusions.

use crate::hash::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P14-F31";
pub const CONTRACT_VERSION: &str = "ids-interpretation-federation/1.0";
pub const RESEARCH_CONTRACT_SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationArtifact {
    pub content_hash: ContentHash,
    pub media_type: String,
    pub scope: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationState {
    Supported,
    Unknown,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBackedResult {
    pub result_id: String,
    pub study_id: String,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub state: InterpretationState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub provenance_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationPlaneRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub results: Vec<EvidenceBackedResult>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub max_concurrency: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationDisposition {
    Admitted,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationPlaneReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub disposition: InterpretationDisposition,
    pub interpretation_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub budget_remaining: u64,
    pub max_concurrency: u32,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: InterpretationArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InterpretationPlaneError {
    #[error("invalid interpretation plane request: {0}")]
    Invalid(String),
    #[error("interpretation plane artifact error: {0}")]
    Artifact(String),
    #[error("interpretation plane serialization error: {0}")]
    Serialization(String),
}

impl InterpretationPlaneReceipt {
    pub fn validate(&self) -> Result<(), InterpretationPlaneError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || (self.interpretation_order.is_empty() && self.blocked_order.is_empty())
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_remaining > self.budget
            || self.max_concurrency == 0
        {
            return Err(InterpretationPlaneError::Invalid(
                "interpretation identity, ordering, budget, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.interpretation_order,
            &self.blocked_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InterpretationPlaneError::Invalid(
                    "interpretation plane ordering is not canonical".into(),
                ));
            }
        }
        if self.artifact.media_type.trim().is_empty() || self.artifact.scope.trim().is_empty() {
            return Err(InterpretationPlaneError::Artifact(
                "interpretation artifact media type and scope are required".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, InterpretationPlaneError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| InterpretationPlaneError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| InterpretationPlaneError::Serialization(error.to_string()))
    }
}

pub fn operate_interpretation_plane(
    request: &InterpretationPlaneRequest,
) -> Result<InterpretationPlaneReceipt, InterpretationPlaneError> {
    validate_request(request)?;
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    let mut results = request.results.clone();
    results.sort_by(|left, right| left.result_id.cmp(&right.result_id));
    for result in &results {
        let evidence_complete = !result.evidence_order.is_empty()
            && result.omissions.is_empty()
            && result.uncertainty.is_empty();
        let cost = result.artifact_order.len() as u64 + result.evidence_order.len() as u64;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let safe_state = result.state == InterpretationState::Supported;
        let admission_ok = request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.federation_allow
            && request.raw_data_local
            && safe_state
            && evidence_complete
            && budget_ok;
        if admission_ok {
            spent = spent.saturating_add(cost);
            admitted.insert(result.result_id.clone());
        } else {
            blocked.insert(result.result_id.clone());
            if result.state != InterpretationState::Supported {
                negative.insert(
                    format!(
                        "result:{}:state-{:?}-cannot-export",
                        result.result_id, result.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if !evidence_complete {
                omissions.insert(format!(
                    "result:{}:protected-omission-or-uncertainty",
                    result.result_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "result:{}:budget-ceiling-exceeded",
                    result.result_id
                ));
            }
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.federation_allow {
        omissions.insert("request:federation-summary-exchange-not-authorized".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if u64::from(request.max_concurrency) > u64::try_from(request.results.len()).unwrap_or(u64::MAX)
    {
        uncertainty.insert("request:concurrency-ceiling-exceeds-result-set".into());
    }
    let interpretation_order = admitted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition =
        if !request.policy_allow || !request.signed_approval || !request.federation_allow {
            InterpretationDisposition::Blocked
        } else if !request.protected_closure || interpretation_order.is_empty() {
            InterpretationDisposition::Unknown
        } else if blocked_order.is_empty() {
            InterpretationDisposition::Admitted
        } else {
            InterpretationDisposition::Partial
        };
    let mut checks = vec![
        "interpretation states, evidence, provenance, policy, authority, locality, and federation gates are explicit".into(),
        "only digest-bound summaries cross the federation boundary; raw experimental bytes remain local".into(),
        "unknown and contradicted interpretations remain blocked or partial and retain negative evidence".into(),
    ];
    checks.sort();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let mut effect_receipts = if matches!(
        disposition,
        InterpretationDisposition::Admitted | InterpretationDisposition::Partial
    ) {
        interpretation_order
            .iter()
            .flat_map(|id| {
                [
                    format!("manage:local-capability:{id}"),
                    format!("exchange:permitted-summary:{id}"),
                ]
            })
            .collect::<Vec<_>>()
    } else {
        vec![format!("block:interpretation-plane:{disposition:?}").to_ascii_lowercase()]
    };
    effect_receipts.sort();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "disposition": disposition,
        "interpretation_order": interpretation_order,
        "blocked_order": blocked_order,
        "replay_identity": request.replay_identity,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = InterpretationArtifact {
        content_hash: ContentHash::of_value(&payload)
            .map_err(|error| InterpretationPlaneError::Artifact(error.to_string()))?,
        media_type: "application/vnd.aurora.ids-interpretation-plane+json".into(),
        scope: format!("ids-interpretation-plane:{}", request.request_id),
    };
    let receipt = InterpretationPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        disposition,
        interpretation_order,
        blocked_order,
        replay_identity: request.replay_identity.clone(),
        budget: request.budget,
        budget_remaining: request.budget.saturating_sub(spent),
        max_concurrency: request.max_concurrency,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &InterpretationPlaneRequest) -> Result<(), InterpretationPlaneError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.results.is_empty()
        || request.max_concurrency == 0
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationPlaneError::Invalid(
            "interpretation identity, results, concurrency, locality, or boundary is required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for result in &request.results {
        if result.result_id.trim().is_empty()
            || result.study_id.trim().is_empty()
            || !ids.insert(result.result_id.clone())
            || result.boundary != PRECLINICAL_BOUNDARY
            || result.modality_order.is_empty()
            || result
                .modality_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || result
                .artifact_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || result
                .evidence_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(InterpretationPlaneError::Invalid(format!(
                "interpretation result {} is invalid or duplicated",
                result.result_id
            )));
        }
        if result
            .modality_order
            .iter()
            .chain(std::iter::once(&result.study_id))
            .chain(std::iter::once(&result.result_id))
            .any(|value| value.chars().any(|character| character.is_control()))
        {
            return Err(InterpretationPlaneError::Invalid(
                "interpretation identities cannot contain control characters".into(),
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

    fn result(id: &str, state: InterpretationState) -> EvidenceBackedResult {
        EvidenceBackedResult {
            result_id: id.into(),
            study_id: format!("study:{id}"),
            modality_order: vec!["imaging".into(), "omics".into()],
            artifact_order: vec![hash(&format!("artifact:{id}"))],
            evidence_order: vec![hash(&format!("evidence:{id}"))],
            state,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            provenance_digest: hash(&format!("provenance:{id}")),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request() -> InterpretationPlaneRequest {
        InterpretationPlaneRequest {
            request_id: "interpretation:plane".into(),
            workflow_id: "workflow:interpretation".into(),
            results: vec![result("result:a", InterpretationState::Supported)],
            replay_identity: hash("replay"),
            budget: 10,
            max_concurrency: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn supported_results_admit_digest_only_summary_exchange() {
        let receipt = operate_interpretation_plane(&request()).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Admitted);
        assert!(receipt
            .effect_receipts
            .iter()
            .all(|effect| effect.contains("summary") || effect.contains("local-capability")));
        assert!(receipt.digest().is_ok());
    }

    #[test]
    fn unknown_result_is_blocked_with_negative_evidence() {
        let mut request = request();
        request.results[0].state = InterpretationState::Unknown;
        let receipt = operate_interpretation_plane(&request).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Unknown);
        assert!(!receipt.negative_evidence.is_empty());
    }

    #[test]
    fn federation_denial_blocks_without_export_effect() {
        let mut request = request();
        request.federation_allow = false;
        let receipt = operate_interpretation_plane(&request).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Blocked);
        assert!(receipt.effect_receipts[0].contains("block:interpretation-plane"));
    }

    #[test]
    fn protected_closure_gap_is_unknown() {
        let mut request = request();
        request.protected_closure = false;
        let receipt = operate_interpretation_plane(&request).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Unknown);
        assert!(!receipt.uncertainty.is_empty());
    }

    #[test]
    fn concurrency_ceiling_gap_is_retained_without_narrowing() {
        let mut request = request();
        request.max_concurrency = 2;
        let receipt = operate_interpretation_plane(&request).unwrap();
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item == "request:concurrency-ceiling-exceeds-result-set"));
    }
}
