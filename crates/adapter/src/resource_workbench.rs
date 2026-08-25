//! Multimodal research-resource discovery workbench.
//!
//! Atlas feature: `AFA-adapter-P05-F18`.
//!
//! This researcher-facing product qualifies typed imaging, omics, protocol, and compute
//! resources against explicit capabilities, origin, availability, trust, and locality rules.
//! It never presents a stale or protected resource as executable and never exports raw records.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P05-F18";
pub const CONTRACT_VERSION: &str = "multimodal-resource-workbench/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceNeed {
    pub need_id: String,
    pub requester: String,
    pub intent: String,
    pub required_capabilities: Vec<String>,
    pub allowed_origins: Vec<String>,
    pub max_results: usize,
    pub federation_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceCandidate {
    pub resource_id: String,
    pub origin: String,
    pub capabilities: Vec<String>,
    pub artifact_digest: ContentHash,
    pub trust_score: f64,
    pub available: bool,
    pub protected: bool,
    pub raw_data_local: bool,
    pub federated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceWorkbenchDisposition {
    Qualified,
    Partial,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedResource {
    pub resource_id: String,
    pub origin: String,
    pub artifact_digest: ContentHash,
    pub rank: usize,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceOmission {
    pub resource_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceWorkbenchReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub need_id: String,
    pub disposition: ResourceWorkbenchDisposition,
    pub qualified_resources: Vec<QualifiedResource>,
    pub omissions: Vec<ResourceOmission>,
    pub checks: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ResourceWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), ResourceWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.need_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(ResourceWorkbenchError::InvalidField(
                "resource workbench identity, checks, or boundary are incomplete".into(),
            ));
        }
        if self
            .qualified_resources
            .iter()
            .enumerate()
            .any(|(index, item)| {
                item.rank != index + 1
                    || item.resource_id.trim().is_empty()
                    || item.origin.trim().is_empty()
                    || item.reasons.is_empty()
            })
        {
            return Err(ResourceWorkbenchError::InvalidField(
                "qualified resource ranking or reasons are invalid".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ResourceWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ResourceWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ResourceWorkbenchError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResourceWorkbenchError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ResourceWorkbenchError {
    #[error("invalid resource workbench field: {0}")]
    InvalidField(String),
    #[error("resource workbench artifact error: {0}")]
    Artifact(String),
    #[error("resource workbench serialization error: {0}")]
    Serialization(String),
}

pub fn discover_resources(
    request_id: &str,
    need: &ResourceNeed,
    candidates: &[ResourceCandidate],
) -> Result<ResourceWorkbenchReceipt, ResourceWorkbenchError> {
    validate_request(request_id, need, candidates)?;
    let mut candidates = candidates.to_vec();
    candidates.sort_by(|left, right| {
        right
            .trust_score
            .total_cmp(&left.trust_score)
            .then_with(|| left.resource_id.cmp(&right.resource_id))
    });
    let mut qualified = Vec::new();
    let mut omissions = Vec::new();
    for candidate in candidates {
        let reason = if !candidate.available {
            Some("resource is unavailable or stale".into())
        } else if candidate.protected {
            Some("resource is protected by institution policy".into())
        } else if !candidate.raw_data_local {
            Some("raw resource data is not institution-local".into())
        } else if candidate.federated && !need.federation_allowed {
            Some("federated origin is not permitted by the need".into())
        } else if !need.allowed_origins.is_empty()
            && !need.allowed_origins.contains(&candidate.origin)
        {
            Some("resource origin is outside the requested scope".into())
        } else if !need
            .required_capabilities
            .iter()
            .all(|capability| candidate.capabilities.contains(capability))
        {
            Some("required resource capability is missing".into())
        } else if !candidate.trust_score.is_finite() || candidate.trust_score < 0.0 {
            Some("resource trust score is not admissible".into())
        } else {
            None
        };
        if let Some(reason) = reason {
            omissions.push(ResourceOmission {
                resource_id: candidate.resource_id,
                reason,
            });
        } else if qualified.len() < need.max_results {
            qualified.push(QualifiedResource {
                resource_id: candidate.resource_id,
                origin: candidate.origin,
                artifact_digest: candidate.artifact_digest,
                rank: qualified.len() + 1,
                reasons: vec![
                    "capability, origin, availability, trust, and locality gates passed".into(),
                ],
            });
        }
    }
    let disposition = if qualified.is_empty() {
        ResourceWorkbenchDisposition::Unknown
    } else if omissions.is_empty() {
        ResourceWorkbenchDisposition::Qualified
    } else {
        ResourceWorkbenchDisposition::Partial
    };
    let mut checks = vec![
        "candidates are deterministically ranked by trust and resource identity".into(),
        "stale, protected, out-of-scope, and non-local resources remain omissions".into(),
        "qualified resources retain content digests without moving raw bytes".into(),
    ];
    checks.push(match disposition {
        ResourceWorkbenchDisposition::Qualified => {
            "all considered resources passed qualification".into()
        }
        ResourceWorkbenchDisposition::Partial => {
            "qualified resources are separated from explicit omissions".into()
        }
        ResourceWorkbenchDisposition::Unknown => {
            "no resource could be qualified; unknown is preserved".into()
        }
        ResourceWorkbenchDisposition::Blocked => "resource discovery is blocked by policy".into(),
    });
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "contract_version": CONTRACT_VERSION, "request_id": request_id, "need_id": need.need_id, "disposition": disposition, "qualified_resources": qualified, "omissions": omissions, "checks": checks, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("resource-workbench:{}", need.need_id),
        "application/vnd.aurora.resource-workbench+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ResourceWorkbenchError::Artifact(error.to_string()))?;
    let receipt = ResourceWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request_id.into(),
        need_id: need.need_id.clone(),
        disposition,
        qualified_resources: qualified,
        omissions,
        checks,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request_id: &str,
    need: &ResourceNeed,
    candidates: &[ResourceCandidate],
) -> Result<(), ResourceWorkbenchError> {
    if request_id.trim().is_empty()
        || need.need_id.trim().is_empty()
        || need.requester.trim().is_empty()
        || need.intent.trim().is_empty()
        || need.required_capabilities.is_empty()
        || need.max_results == 0
        || candidates.is_empty()
    {
        return Err(ResourceWorkbenchError::InvalidField(
            "resource need identity, capabilities, result bound, and candidates are required"
                .into(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for candidate in candidates {
        if candidate.resource_id.trim().is_empty()
            || candidate.origin.trim().is_empty()
            || !ids.insert(candidate.resource_id.clone())
        {
            return Err(ResourceWorkbenchError::InvalidField(
                "resource identities must be non-empty and unique".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_resource_is_omitted() {
        let receipt = discover_resources(
            "request:resources",
            &ResourceNeed {
                need_id: "need:imaging".into(),
                requester: "researcher".into(),
                intent: "find local imaging".into(),
                required_capabilities: vec!["imaging".into()],
                allowed_origins: vec!["site:a".into()],
                max_results: 2,
                federation_allowed: false,
            },
            &[ResourceCandidate {
                resource_id: "resource:protected".into(),
                origin: "site:a".into(),
                capabilities: vec!["imaging".into()],
                artifact_digest: ContentHash::of_bytes(b"protected"),
                trust_score: 0.9,
                available: true,
                protected: true,
                raw_data_local: true,
                federated: false,
            }],
        )
        .unwrap();
        assert_eq!(receipt.disposition, ResourceWorkbenchDisposition::Unknown);
        assert_eq!(
            receipt.omissions[0].reason,
            "resource is protected by institution policy"
        );
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
