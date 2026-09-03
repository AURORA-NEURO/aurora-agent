//! Federated continual resource-discovery control plane.
//!
//! Atlas feature: `AFA-weave-P05-F32`.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-weave-P05-F32";
pub const CONTRACT_VERSION: &str = "federated-resource-control-plane/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceControlPlaneRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_ids: Vec<String>,
    pub requested_resource_ids: Vec<String>,
    pub qualified_resource_ids: Vec<String>,
    pub qualification_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub approval_reference: Option<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceControlDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceControlPlaneReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_ids: Vec<String>,
    pub qualified_resource_ids: Vec<String>,
    pub disposition: ResourceControlDisposition,
    pub qualification_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ResourceControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), ResourceControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_ids.len() < 2
            || self.checks.is_empty()
        {
            return Err(ResourceControlError::InvalidField(
                "resource control identity, boundary, institutions, or checks are incomplete"
                    .into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ResourceControlError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ResourceControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ResourceControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResourceControlError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ResourceControlError {
    #[error("invalid resource control field: {0}")]
    InvalidField(String),
    #[error("resource control artifact error: {0}")]
    Artifact(String),
    #[error("resource control serialization error: {0}")]
    Serialization(String),
}

pub fn operate_resource_control_plane(
    request: &ResourceControlPlaneRequest,
) -> Result<ResourceControlPlaneReceipt, ResourceControlError> {
    validate_request(request)?;
    let mut institutions = request.institution_ids.clone();
    institutions.sort();
    institutions.dedup();
    let mut qualified = request.qualified_resource_ids.clone();
    qualified.sort();
    qualified.dedup();
    let missing = request
        .requested_resource_ids
        .iter()
        .filter(|id| !qualified.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    let mut checks = vec![
        "institution identities and resource ids are canonicalized".to_string(),
        "raw resource records remain institution-local".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || request.approval_reference.is_none()
    {
        checks.push(
            "policy, approval, or protected closure prevented control-plane admission".into(),
        );
        ResourceControlDisposition::Blocked
    } else if request.qualification_digest.is_none() || !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|id| format!("resource qualification unavailable: {id}")),
        );
        if request.qualification_digest.is_none() {
            omissions.push("qualification receipt is absent".into());
        }
        checks.push("incomplete qualification remains unknown rather than executable".into());
        ResourceControlDisposition::Unknown
    } else {
        checks.push("qualified resources and approval receipt passed".into());
        ResourceControlDisposition::Passed
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "contract_version": CONTRACT_VERSION, "request_id": request.request_id, "federation_id": request.federation_id, "institution_ids": institutions, "qualified_resource_ids": qualified, "disposition": disposition, "qualification_digest": request.qualification_digest, "checks": checks, "omissions": omissions, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("resource-control-plane:{}", request.request_id),
        "application/vnd.aurora.resource-control-plane+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ResourceControlError::Artifact(error.to_string()))?;
    let receipt = ResourceControlPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_ids: institutions,
        qualified_resource_ids: qualified,
        disposition,
        qualification_digest: request.qualification_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ResourceControlPlaneRequest) -> Result<(), ResourceControlError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.institution_ids.len() < 2
        || request.requested_resource_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ResourceControlError::InvalidField(
            "resource control identity, institutions, resources, and boundary are required".into(),
        ));
    }
    if request.policy_decision == PolicyDecision::Allow && request.approval_reference.is_none() {
        return Err(ResourceControlError::InvalidField(
            "A2 resource control requires an approval reference".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_qualification_is_unknown() {
        let receipt = operate_resource_control_plane(&ResourceControlPlaneRequest {
            request_id: "request:resources".into(),
            federation_id: "federation:resources".into(),
            institution_ids: vec!["site:a".into(), "site:b".into()],
            requested_resource_ids: vec!["resource:a".into()],
            qualified_resource_ids: vec![],
            qualification_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:a".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, ResourceControlDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
