//! Federated continual resource-discovery workbench.
//!
//! Atlas feature: `AFA-fiber-P05-F20`.
//!
//! This is a researcher-facing qualification surface, not an unbounded search engine. It ranks
//! only typed registry candidates that satisfy the request's capabilities and locality policy;
//! stale, unavailable, protected, or out-of-scope candidates remain explicit omissions.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-fiber-P05-F20";
pub const FEATURE_VERSION: &str = "0.1.0";
pub const MAX_CANDIDATES: usize = 4096;
pub const MAX_RESULTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceNeed {
    pub need_id: String,
    pub requester: String,
    pub intent: String,
    pub allowed_origins: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub max_results: usize,
    pub federation_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAvailability {
    Available,
    Stale,
    Protected,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceCandidate {
    pub resource_id: String,
    pub origin: String,
    pub capabilities: Vec<String>,
    pub artifact_digest: ContentHash,
    pub trust_score: f64,
    pub availability: ResourceAvailability,
    pub raw_data_local: bool,
    pub federated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceDiscoveryDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
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
pub struct QualifiedResourceSet {
    pub schema_version: String,
    pub feature_id: String,
    pub need_id: String,
    pub requester: String,
    pub disposition: ResourceDiscoveryDisposition,
    pub considered_candidates: usize,
    pub qualified_count: usize,
    pub resources: Vec<QualifiedResource>,
    pub omissions: Vec<ResourceOmission>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl QualifiedResourceSet {
    pub fn validate(&self) -> Result<(), ResourceWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.need_id.trim().is_empty()
            || self.requester.trim().is_empty()
        {
            return Err(ResourceWorkbenchError::InvalidField(
                "schema, identity, requester, or boundary".into(),
            ));
        }
        if self.considered_candidates == 0
            || self.qualified_count != self.resources.len()
            || self.reasons.is_empty()
            || self.resources.iter().any(|resource| {
                resource.resource_id.trim().is_empty()
                    || resource.origin.trim().is_empty()
                    || resource.reasons.is_empty()
                    || resource.rank == 0
            })
        {
            return Err(ResourceWorkbenchError::InvalidField(
                "candidate counts, resources, or reasons".into(),
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
    #[error("resource candidate set is too large: {0} > {MAX_CANDIDATES}")]
    TooManyCandidates(usize),
    #[error("duplicate resource candidate {0}")]
    DuplicateResource(String),
    #[error("resource workbench artifact error: {0}")]
    Artifact(String),
    #[error("resource workbench serialization error: {0}")]
    Serialization(String),
}

pub fn resource_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "fiber".into(),
        consumers: ["context compiler engineer".into(), "preclinical researcher".into()].into(),
        behavior: "qualifies typed local and permitted federated research resources against capability, origin, availability, trust, and locality constraints with deterministic ranking".into(),
        value: "turns continual resource discovery into an auditable workbench result without exposing protected raw data or hiding unavailable candidates".into(),
        inputs: vec![TypedPort { name: "resource_need_and_candidates".into(), schema: "ResourceNeed4@1 + ResourceCandidate@1".into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_resource_set".into(), schema: "QualifiedResourceSet5@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(),
        permissions: ["view:authorized-research-state".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn discover_resources(
    need: &ResourceNeed,
    candidates: &[ResourceCandidate],
) -> Result<QualifiedResourceSet, ResourceWorkbenchError> {
    validate_need(need, candidates)?;
    let mut candidates = candidates.to_vec();
    candidates.sort_by(|left, right| {
        right
            .trust_score
            .total_cmp(&left.trust_score)
            .then(left.resource_id.cmp(&right.resource_id))
    });
    let mut qualified = Vec::new();
    let mut omissions = Vec::new();
    let allowed_origins: BTreeSet<&str> = need.allowed_origins.iter().map(String::as_str).collect();
    for candidate in &candidates {
        let reason = if candidate.availability != ResourceAvailability::Available {
            Some(format!(
                "candidate availability is {:?}",
                candidate.availability
            ))
        } else if !candidate.raw_data_local {
            Some("raw research data is not institution-local".into())
        } else if candidate.federated && !need.federation_allowed {
            Some("federated origin is not permitted by this need".into())
        } else if !allowed_origins.is_empty()
            && !allowed_origins.contains(candidate.origin.as_str())
        {
            Some("origin is outside the requested scope".into())
        } else if !need
            .required_capabilities
            .iter()
            .all(|required| candidate.capabilities.contains(required))
        {
            Some("required capability is missing".into())
        } else if !candidate.trust_score.is_finite() || candidate.trust_score < 0.0 {
            Some("trust score is not admissible".into())
        } else {
            None
        };
        if let Some(reason) = reason {
            omissions.push(ResourceOmission {
                resource_id: candidate.resource_id.clone(),
                reason,
            });
        } else if qualified.len() < need.max_results {
            qualified.push(QualifiedResource {
                resource_id: candidate.resource_id.clone(),
                origin: candidate.origin.clone(),
                artifact_digest: candidate.artifact_digest.clone(),
                rank: qualified.len() + 1,
                reasons: vec![
                    "capabilities, scope, locality, availability, and trust gates passed".into(),
                ],
            });
        } else {
            omissions.push(ResourceOmission {
                resource_id: candidate.resource_id.clone(),
                reason: "result limit reached; candidate remains unselected".into(),
            });
        }
    }
    let disposition = if qualified.is_empty() {
        if omissions.iter().any(|omission| {
            omission.reason.contains("protected") || omission.reason.contains("local")
        }) {
            ResourceDiscoveryDisposition::Blocked
        } else {
            ResourceDiscoveryDisposition::Unknown
        }
    } else if omissions.is_empty() {
        ResourceDiscoveryDisposition::Qualified
    } else {
        ResourceDiscoveryDisposition::Partial
    };
    let reasons = if qualified.is_empty() {
        vec!["no candidate satisfied the typed resource need; omissions remain explicit".into()]
    } else {
        vec![format!(
            "{} of {} candidates qualified; {} omissions retained",
            qualified.len(),
            candidates.len(),
            omissions.len()
        )]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "need_id": need.need_id,
        "requester": need.requester,
        "disposition": disposition,
        "considered_candidates": candidates.len(),
        "qualified_count": qualified.len(),
        "resources": qualified,
        "omissions": omissions,
        "reasons": reasons,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        "qualified-resource-set",
        "application/vnd.aurora.qualified-resource-set+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ResourceWorkbenchError::Artifact(error.to_string()))?;
    let receipt = QualifiedResourceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        need_id: need.need_id.clone(),
        requester: need.requester.clone(),
        disposition,
        considered_candidates: candidates.len(),
        qualified_count: qualified.len(),
        resources: serde_json::from_value(payload["resources"].clone())
            .map_err(|error| ResourceWorkbenchError::Serialization(error.to_string()))?,
        omissions: serde_json::from_value(payload["omissions"].clone())
            .map_err(|error| ResourceWorkbenchError::Serialization(error.to_string()))?,
        reasons: serde_json::from_value(payload["reasons"].clone())
            .map_err(|error| ResourceWorkbenchError::Serialization(error.to_string()))?,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_need(
    need: &ResourceNeed,
    candidates: &[ResourceCandidate],
) -> Result<(), ResourceWorkbenchError> {
    if need.need_id.trim().is_empty()
        || need.requester.trim().is_empty()
        || need.intent.trim().is_empty()
        || need.required_capabilities.is_empty()
        || need.max_results == 0
        || need.max_results > MAX_RESULTS
        || candidates.is_empty()
    {
        return Err(ResourceWorkbenchError::InvalidField(
            "need identity, intent, capabilities, result limit, and candidates are required".into(),
        ));
    }
    if candidates.len() > MAX_CANDIDATES {
        return Err(ResourceWorkbenchError::TooManyCandidates(candidates.len()));
    }
    let mut ids = BTreeSet::new();
    for candidate in candidates {
        if candidate.resource_id.trim().is_empty()
            || candidate.origin.trim().is_empty()
            || !ids.insert(candidate.resource_id.clone())
        {
            return Err(ResourceWorkbenchError::DuplicateResource(
                candidate.resource_id.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn need() -> ResourceNeed {
        ResourceNeed {
            need_id: "need:organoid".into(),
            requester: "researcher:alice".into(),
            intent: "find image and rna resources".into(),
            allowed_origins: vec!["site-a".into(), "site-b".into()],
            required_capabilities: vec!["imaging".into(), "rna".into()],
            max_results: 2,
            federation_allowed: true,
        }
    }

    fn candidate(id: &str, score: f64) -> ResourceCandidate {
        ResourceCandidate {
            resource_id: id.into(),
            origin: "site-a".into(),
            capabilities: vec!["imaging".into(), "rna".into()],
            artifact_digest: ContentHash::of_bytes(id.as_bytes()),
            trust_score: score,
            availability: ResourceAvailability::Available,
            raw_data_local: true,
            federated: false,
        }
    }

    #[test]
    fn ranking_is_deterministic_and_omissions_are_retained() {
        let result = discover_resources(
            &need(),
            &[candidate("resource:b", 0.7), candidate("resource:a", 0.9)],
        )
        .unwrap();
        assert_eq!(result.disposition, ResourceDiscoveryDisposition::Qualified);
        assert_eq!(result.resources[0].resource_id, "resource:a");
        assert!(result.omissions.is_empty());
    }

    #[test]
    fn protected_candidate_blocks_when_no_local_resource_qualifies() {
        let mut candidate = candidate("resource:protected", 1.0);
        candidate.raw_data_local = false;
        let result = discover_resources(&need(), &[candidate]).unwrap();
        assert_eq!(result.disposition, ResourceDiscoveryDisposition::Blocked);
        assert_eq!(result.omissions.len(), 1);
    }
}
