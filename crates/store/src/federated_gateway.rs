//! Federated continual knowledge-store interoperability gateway.
//!
//! Atlas feature: `AFA-store-P04-F24`.
//!
//! The gateway exchanges canonical store manifests, capability metadata, and content digests
//! rather than raw world records. It makes schema, locality, policy, and omission decisions
//! explicit before a consortium accepts a knowledge representation from another institution.

use crate::build::{StoreManifest, STORE_SCHEMA_VERSION};
use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-store-P04-F24";
pub const CONTRACT_VERSION: &str = "federated-knowledge-gateway/1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedKnowledgeGatewayRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_ids: Vec<String>,
    pub interoperability_profile: String,
    pub manifest: StoreManifest,
    pub permitted_tags: Vec<String>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedKnowledgeGatewayReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub interoperability_profile: String,
    pub institution_ids: Vec<String>,
    pub disposition: GatewayDisposition,
    pub manifest_digest: ContentHash,
    pub permitted_tags: Vec<String>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl FederatedKnowledgeGatewayReceipt {
    pub fn validate(&self) -> Result<(), GatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.interoperability_profile.trim().is_empty()
            || self.institution_ids.len() < 2
            || self.institution_ids.iter().any(|id| id.trim().is_empty())
            || self
                .institution_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.checks.is_empty()
        {
            return Err(GatewayError::InvalidField(
                "gateway identity, profile, institutions, locality, or checks are incomplete"
                    .into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| GatewayError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, GatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| GatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| GatewayError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("invalid federated knowledge gateway field: {0}")]
    InvalidField(String),
    #[error("federated knowledge gateway artifact error: {0}")]
    Artifact(String),
    #[error("federated knowledge gateway serialization error: {0}")]
    Serialization(String),
}

pub fn admit_federated_knowledge(
    request: &FederatedKnowledgeGatewayRequest,
) -> Result<FederatedKnowledgeGatewayReceipt, GatewayError> {
    validate_request(request)?;
    let manifest_value = serde_json::to_value(&request.manifest)
        .map_err(|error| GatewayError::Serialization(error.to_string()))?;
    let manifest_digest = ContentHash::of_value(&manifest_value)
        .map_err(|error| GatewayError::Serialization(error.to_string()))?;
    let mut checks = vec![
        "store manifest schema is compatible".to_string(),
        "manifest digest and indexed counts are typed".to_string(),
        "raw world records remain institution-local".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented gateway admission".into());
        GatewayDisposition::Blocked
    } else if request.permitted_tags.is_empty() || request.manifest.tag_counts.is_empty() {
        omissions.push("no permitted tag projection was supplied for federation".into());
        checks.push(
            "missing tag projection remains unknown rather than an unrestricted export".into(),
        );
        GatewayDisposition::Unknown
    } else {
        checks.push("canonical manifest and permitted tag projection passed".into());
        GatewayDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "interoperability_profile": request.interoperability_profile,
        "institution_ids": request.institution_ids,
        "disposition": disposition,
        "manifest_digest": manifest_digest,
        "permitted_tags": request.permitted_tags,
        "checks": checks,
        "omissions": omissions,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-knowledge-gateway:{}", request.request_id),
        "application/vnd.aurora.federated-knowledge-gateway+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| GatewayError::Artifact(error.to_string()))?;
    let receipt = FederatedKnowledgeGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        interoperability_profile: request.interoperability_profile.clone(),
        institution_ids: request.institution_ids.clone(),
        disposition,
        manifest_digest,
        permitted_tags: request.permitted_tags.clone(),
        checks,
        omissions,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederatedKnowledgeGatewayRequest) -> Result<(), GatewayError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.interoperability_profile.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || request.institution_ids.len() < 2
        || request
            .institution_ids
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(GatewayError::InvalidField(
            "gateway identity, profile, institutions, locality, and boundary are required".into(),
        ));
    }
    let unique: BTreeSet<_> = request.institution_ids.iter().collect();
    if unique.len() != request.institution_ids.len() {
        return Err(GatewayError::InvalidField(
            "institution ids must be unique".into(),
        ));
    }
    if request.manifest.schema_version != STORE_SCHEMA_VERSION {
        return Err(GatewayError::InvalidField(
            "store manifest schema is incompatible".into(),
        ));
    }
    if request.manifest.world_id.trim().is_empty()
        || request.manifest.world_sha256.len() != 64
        || request.manifest.total_facts == 0
    {
        return Err(GatewayError::InvalidField(
            "store manifest identity, digest, and indexed facts are required".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn manifest() -> StoreManifest {
        StoreManifest {
            schema_version: STORE_SCHEMA_VERSION.into(),
            world_id: "world:organoid".into(),
            world_sha256: "a".repeat(64),
            total_facts: 2,
            total_factors: 1,
            tag_counts: BTreeMap::from([(String::from("imaging"), 2)]),
            events: vec![],
            description: Some("local manifest".into()),
        }
    }

    fn request(tags: Vec<String>) -> FederatedKnowledgeGatewayRequest {
        FederatedKnowledgeGatewayRequest {
            request_id: "request:gateway".into(),
            federation_id: "federation:preclinical".into(),
            institution_ids: vec!["site:a".into(), "site:b".into()],
            interoperability_profile: "ro-crate+prov-o:1".into(),
            manifest: manifest(),
            permitted_tags: tags,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn missing_projection_is_unknown_not_unrestricted() {
        let receipt = admit_federated_knowledge(&request(vec![])).unwrap();
        assert_eq!(receipt.disposition, GatewayDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn policy_denial_blocks_gateway() {
        let mut input = request(vec!["imaging".into()]);
        input.policy_decision = PolicyDecision::Deny;
        let receipt = admit_federated_knowledge(&input).unwrap();
        assert_eq!(receipt.disposition, GatewayDisposition::Blocked);
    }
}
