//! Prospective high-throughput mechanism-exploration federation control plane.
//!
//! Atlas feature: `AFA-adapter-P08-F31`.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P08-F31";
pub const CONTRACT_VERSION: &str = "federated-mechanism-control-plane/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismControlPlaneRequest {
    pub request_id: String,
    pub federation_id: String,
    pub question_id: String,
    pub required_candidate_ids: Vec<String>,
    pub admitted_candidate_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub approval_reference: Option<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismControlDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismControlPlaneReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub question_id: String,
    pub admitted_candidate_ids: Vec<String>,
    pub disposition: MechanismControlDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl MechanismControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), MechanismControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.question_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(MechanismControlError::InvalidField(
                "mechanism control identity, boundary, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MechanismControlError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MechanismControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MechanismControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MechanismControlError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum MechanismControlError {
    #[error("invalid mechanism control field: {0}")]
    InvalidField(String),
    #[error("mechanism control artifact error: {0}")]
    Artifact(String),
    #[error("mechanism control serialization error: {0}")]
    Serialization(String),
}

pub fn operate_mechanism_control_plane(
    request: &MechanismControlPlaneRequest,
) -> Result<MechanismControlPlaneReceipt, MechanismControlError> {
    validate_request(request)?;
    let mut admitted = request.admitted_candidate_ids.clone();
    admitted.sort();
    admitted.dedup();
    let missing = request
        .required_candidate_ids
        .iter()
        .filter(|candidate| !admitted.contains(candidate))
        .cloned()
        .collect::<Vec<_>>();
    let mut checks = vec![
        "mechanism candidate identities are canonicalized".to_string(),
        "raw institution-local evidence remains outside the federation envelope".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || request.approval_reference.is_none()
    {
        checks.push("policy, approval, or protected closure blocked mechanism admission".into());
        MechanismControlDisposition::Blocked
    } else if request.evidence_receipt_digest.is_none() || !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|candidate| format!("required mechanism candidate unavailable: {candidate}")),
        );
        if request.evidence_receipt_digest.is_none() {
            omissions.push("mechanism evidence receipt is absent".into());
        }
        checks.push("incomplete mechanism evidence remains unknown rather than admitted".into());
        MechanismControlDisposition::Unknown
    } else {
        checks.push("candidate set, evidence receipt, and approval passed".into());
        MechanismControlDisposition::Passed
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "contract_version": CONTRACT_VERSION, "request_id": request.request_id, "federation_id": request.federation_id, "question_id": request.question_id, "admitted_candidate_ids": admitted, "disposition": disposition, "evidence_receipt_digest": request.evidence_receipt_digest, "checks": checks, "omissions": omissions, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("mechanism-control-plane:{}", request.question_id),
        "application/vnd.aurora.mechanism-control-plane+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MechanismControlError::Artifact(error.to_string()))?;
    let receipt = MechanismControlPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        question_id: request.question_id.clone(),
        admitted_candidate_ids: admitted,
        disposition,
        evidence_receipt_digest: request.evidence_receipt_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &MechanismControlPlaneRequest) -> Result<(), MechanismControlError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.question_id.trim().is_empty()
        || request.required_candidate_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MechanismControlError::InvalidField(
            "mechanism identity, candidates, and boundary are required".into(),
        ));
    }
    if request.policy_decision == PolicyDecision::Allow && request.approval_reference.is_none() {
        return Err(MechanismControlError::InvalidField(
            "A2 mechanism control requires an approval reference".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_candidate_is_unknown() {
        let receipt = operate_mechanism_control_plane(&MechanismControlPlaneRequest {
            request_id: "request:mechanism".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into(), "candidate:b".into()],
            admitted_candidate_ids: vec!["candidate:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:mechanism".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, MechanismControlDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
