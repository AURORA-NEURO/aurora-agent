//! Versioned MCP contract for federated continual resource discovery.
//!
//! Atlas feature: `AFA-mcp-P05-F08`.
//!
//! The MCP crate owns the compatibility envelope while FIBER owns the qualification semantics.
//! This keeps protocol evolution independently deployable: a client can negotiate a profile and
//! receive migration notes without changing the deterministic resource-ranking kernel.

use bioprism_fiber::{discover_resources, QualifiedResourceSet, ResourceCandidate, ResourceNeed};
use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-mcp-P05-F08";
pub const CONTRACT_VERSION: &str = "aurora-mcp-resource-discovery/2.0";
pub const MAX_COMPATIBILITY_PROFILE_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceDiscoveryContractRequest {
    pub schema_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requested_by: String,
    pub compatibility_profile: String,
    pub need: ResourceNeed,
    pub candidates: Vec<ResourceCandidate>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceDiscoveryContractResponse {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub requested_by: String,
    pub compatibility_profile: String,
    pub result: QualifiedResourceSet,
    pub migration_notes: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ResourceDiscoveryContractResponse {
    pub fn validate(&self) -> Result<(), ResourceDiscoveryContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.requested_by.trim().is_empty()
            || self.compatibility_profile.trim().is_empty()
            || self.compatibility_profile.len() > MAX_COMPATIBILITY_PROFILE_BYTES
            || self.migration_notes.is_empty()
        {
            return Err(ResourceDiscoveryContractError::InvalidField(
                "schema, identity, compatibility, migration, or boundary".into(),
            ));
        }
        self.result.validate()?;
        self.artifact
            .validate_metadata()
            .map_err(|error| ResourceDiscoveryContractError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ResourceDiscoveryContractError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ResourceDiscoveryContractError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResourceDiscoveryContractError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ResourceDiscoveryContractError {
    #[error("invalid resource discovery contract field: {0}")]
    InvalidField(String),
    #[error("resource discovery contract result error: {0}")]
    Result(#[from] bioprism_fiber::ResourceWorkbenchError),
    #[error("resource discovery contract artifact error: {0}")]
    Artifact(String),
    #[error("resource discovery contract serialization error: {0}")]
    Serialization(String),
}

pub fn compile_resource_discovery_contract_v2(
    request: &ResourceDiscoveryContractRequest,
) -> Result<ResourceDiscoveryContractResponse, ResourceDiscoveryContractError> {
    validate_request(request)?;
    let result = discover_resources(&request.need, &request.candidates)?;
    let migration_notes = vec![
        "v2 keeps the v1 ResourceNeed and QualifiedResourceSet semantic fields stable".into(),
        "omissions, locality, federation, and protected-closure posture are never dropped during migration".into(),
    ];
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "requested_by": request.requested_by,
        "compatibility_profile": request.compatibility_profile,
        "result": result,
        "migration_notes": migration_notes,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        "mcp-resource-discovery-contract",
        "application/vnd.aurora.mcp.resource-discovery+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ResourceDiscoveryContractError::Artifact(error.to_string()))?;
    let response = ResourceDiscoveryContractResponse {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        requested_by: request.requested_by.clone(),
        compatibility_profile: request.compatibility_profile.clone(),
        result,
        migration_notes,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    response.validate()?;
    Ok(response)
}

fn validate_request(
    request: &ResourceDiscoveryContractRequest,
) -> Result<(), ResourceDiscoveryContractError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.feature_id != FEATURE_ID
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request_id.trim().is_empty()
        || request.requested_by.trim().is_empty()
        || request.compatibility_profile.trim().is_empty()
        || request.compatibility_profile.len() > MAX_COMPATIBILITY_PROFILE_BYTES
    {
        return Err(ResourceDiscoveryContractError::InvalidField(
            "request schema, identity, compatibility, or boundary".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_fiber::{ResourceAvailability, ResourceCandidate};
    use bioprism_ids::ContentHash;

    fn request() -> ResourceDiscoveryContractRequest {
        ResourceDiscoveryContractRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            feature_id: FEATURE_ID.into(),
            request_id: "request:resource-v2".into(),
            requested_by: "admin:consortium".into(),
            compatibility_profile: "qualified-resource-set/v1".into(),
            need: ResourceNeed {
                need_id: "need:imaging".into(),
                requester: "researcher:alice".into(),
                intent: "find local imaging resource".into(),
                allowed_origins: vec!["site-a".into()],
                required_capabilities: vec!["imaging".into()],
                max_results: 1,
                federation_allowed: false,
            },
            candidates: vec![ResourceCandidate {
                resource_id: "resource:image-a".into(),
                origin: "site-a".into(),
                capabilities: vec!["imaging".into()],
                artifact_digest: ContentHash::of_bytes(b"image-a"),
                trust_score: 0.9,
                availability: ResourceAvailability::Available,
                raw_data_local: true,
                federated: false,
            }],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn compatibility_envelope_is_deterministic() {
        let response = compile_resource_discovery_contract_v2(&request()).unwrap();
        assert_eq!(response.contract_version, CONTRACT_VERSION);
        assert_eq!(response.result.qualified_count, 1);
        assert_eq!(response.digest().unwrap(), response.digest().unwrap());
    }

    #[test]
    fn malformed_profile_fails_closed() {
        let mut request = request();
        request.compatibility_profile = "x".repeat(MAX_COMPATIBILITY_PROFILE_BYTES + 1);
        assert!(compile_resource_discovery_contract_v2(&request).is_err());
    }
}
