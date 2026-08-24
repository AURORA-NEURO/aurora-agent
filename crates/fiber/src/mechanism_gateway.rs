//! Federated continual mechanism-exploration interoperability gateway.
//!
//! Atlas feature: `AFA-fiber-P08-F24`.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-fiber-P08-F24";
pub const CONTRACT_VERSION: &str = "federated-mechanism-interoperability-gateway/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismGatewayRequest {
    pub request_id: String,
    pub federation_id: String,
    pub source_profile: String,
    pub target_profile: String,
    pub required_candidate_ids: Vec<String>,
    pub projected_candidate_ids: Vec<String>,
    pub projection_digest: Option<ContentHash>,
    pub interoperability_profile: String,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismGatewayDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismGatewayReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub source_profile: String,
    pub target_profile: String,
    pub projected_candidate_ids: Vec<String>,
    pub interoperability_profile: String,
    pub disposition: MechanismGatewayDisposition,
    pub projection_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl MechanismGatewayReceipt {
    pub fn validate(&self) -> Result<(), MechanismGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.source_profile.trim().is_empty()
            || self.target_profile.trim().is_empty()
            || self.interoperability_profile.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(MechanismGatewayError::InvalidField(
                "mechanism gateway identity, profiles, boundary, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MechanismGatewayError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MechanismGatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MechanismGatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MechanismGatewayError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum MechanismGatewayError {
    #[error("invalid mechanism gateway field: {0}")]
    InvalidField(String),
    #[error("mechanism gateway artifact error: {0}")]
    Artifact(String),
    #[error("mechanism gateway serialization error: {0}")]
    Serialization(String),
}

pub fn admit_mechanism_gateway(
    request: &MechanismGatewayRequest,
) -> Result<MechanismGatewayReceipt, MechanismGatewayError> {
    validate_request(request)?;
    let mut projected = request.projected_candidate_ids.clone();
    projected.sort();
    projected.dedup();
    let missing = request
        .required_candidate_ids
        .iter()
        .filter(|candidate| !projected.contains(candidate))
        .cloned()
        .collect::<Vec<_>>();
    let mut checks = vec![
        "source and target profiles are explicit and canonicalized".to_string(),
        "raw preclinical data remains institution-local".to_string(),
        "federation exchanges projected candidate metadata rather than source records".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local
    {
        checks.push(
            "policy, locality, or protected closure blocked interoperability admission".into(),
        );
        MechanismGatewayDisposition::Blocked
    } else if request.projection_digest.is_none() || !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|candidate| format!("candidate projection unavailable: {candidate}")),
        );
        if request.projection_digest.is_none() {
            omissions.push("projection receipt is absent".into());
        }
        checks.push(
            "incomplete candidate projection remains unknown rather than interoperable".into(),
        );
        MechanismGatewayDisposition::Unknown
    } else {
        checks.push(
            "candidate projection, interoperability profile, and evidence digest passed".into(),
        );
        MechanismGatewayDisposition::Passed
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "contract_version": CONTRACT_VERSION, "request_id": request.request_id, "federation_id": request.federation_id, "source_profile": request.source_profile, "target_profile": request.target_profile, "projected_candidate_ids": projected, "interoperability_profile": request.interoperability_profile, "disposition": disposition, "projection_digest": request.projection_digest, "checks": checks, "omissions": omissions, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("mechanism-interoperability-gateway:{}", request.request_id),
        "application/vnd.aurora.mechanism-interoperability-gateway+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MechanismGatewayError::Artifact(error.to_string()))?;
    let receipt = MechanismGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        source_profile: request.source_profile.clone(),
        target_profile: request.target_profile.clone(),
        projected_candidate_ids: projected,
        interoperability_profile: request.interoperability_profile.clone(),
        disposition,
        projection_digest: request.projection_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &MechanismGatewayRequest) -> Result<(), MechanismGatewayError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.source_profile.trim().is_empty()
        || request.target_profile.trim().is_empty()
        || request.interoperability_profile.trim().is_empty()
        || request.required_candidate_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MechanismGatewayError::InvalidField(
            "mechanism gateway identity, profiles, candidates, and boundary are required".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_projection_is_unknown() {
        let receipt = admit_mechanism_gateway(&MechanismGatewayRequest {
            request_id: "request:gateway".into(),
            federation_id: "federation:mechanism".into(),
            source_profile: "mechanism-v1".into(),
            target_profile: "mechanism-v2".into(),
            required_candidate_ids: vec!["candidate:a".into()],
            projected_candidate_ids: vec![],
            projection_digest: None,
            interoperability_profile: "ro-crate+prov-o:1".into(),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, MechanismGatewayDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
