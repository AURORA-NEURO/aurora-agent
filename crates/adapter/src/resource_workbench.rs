//! Multimodal research-resource discovery workbench.
//!
//! Atlas feature: `AFA-adapter-P05-F18`.
//!
//! This researcher-facing product qualifies typed imaging, omics, protocol, and compute
//! resources against explicit capabilities, origin, availability, trust, and locality rules.
//! It never presents a stale or protected resource as executable and never exports raw records.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P05-F18";
pub const CONTRACT_VERSION: &str = "multimodal-resource-workbench/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_CANDIDATES: usize = 8192;
const MAX_ITEMS: usize = 16384;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceDiscoveryRequest {
    pub request_id: String,
    pub need: ResourceNeed,
    pub candidates: Vec<ResourceCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceWorkbenchDisposition {
    Qualified,
    Partial,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualifiedResource {
    pub resource_id: String,
    pub origin: String,
    pub artifact_digest: ContentHash,
    pub trust_score: f64,
    pub rank: usize,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceOmission {
    pub resource_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceWorkbenchReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub input: ResourceDiscoveryRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub need_id: String,
    pub requester: String,
    pub intent: String,
    pub required_capabilities: Vec<String>,
    pub allowed_origins: Vec<String>,
    pub max_results: usize,
    pub federation_allowed: bool,
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
        validate_text("request_id", &self.request_id)?;
        validate_text("need_id", &self.need_id)?;
        validate_text("requester", &self.requester)?;
        validate_text("intent", &self.intent)?;
        if self.max_results == 0 || self.max_results > MAX_CANDIDATES {
            return Err(ResourceWorkbenchError::InvalidField(
                "resource result bound is outside its contract".into(),
            ));
        }
        validate_unique_strings(
            &self.required_capabilities,
            "required_capabilities",
            MAX_ITEMS,
        )?;
        validate_unique_strings(&self.allowed_origins, "allowed_origins", MAX_ITEMS)?;
        if self.required_capabilities.is_empty() {
            return Err(ResourceWorkbenchError::InvalidField(
                "resource qualification requires capabilities".into(),
            ));
        }
        if self.qualified_resources.len() > self.max_results
            || self.qualified_resources.len() > MAX_CANDIDATES
        {
            return Err(ResourceWorkbenchError::InvalidField(
                "qualified resource count exceeds the requested bound".into(),
            ));
        }
        if self.qualified_resources.windows(2).any(|pair| {
            pair[0].trust_score < pair[1].trust_score
                || (pair[0].trust_score == pair[1].trust_score
                    && pair[0].resource_id >= pair[1].resource_id)
        }) {
            return Err(ResourceWorkbenchError::InvalidField(
                "qualified resources are not ordered by trust and resource identity".into(),
            ));
        }
        let mut qualified_ids = std::collections::BTreeSet::new();
        for (index, item) in self.qualified_resources.iter().enumerate() {
            validate_text("qualified.resource_id", &item.resource_id)?;
            validate_text("qualified.origin", &item.origin)?;
            if item.rank != index + 1
                || !qualified_ids.insert(item.resource_id.clone())
                || !item.trust_score.is_finite()
                || !(0.0..=1.0).contains(&item.trust_score)
                || item.artifact_digest == ContentHash::of_bytes(b"")
            {
                return Err(ResourceWorkbenchError::InvalidField(
                    "qualified resource ranking, identity, trust, or digest is invalid".into(),
                ));
            }
            validate_sorted_strings(&item.reasons, "qualified.reasons")?;
            if item.reasons
                != vec![String::from(
                    "capability, origin, availability, trust, and locality gates passed",
                )]
            {
                return Err(ResourceWorkbenchError::InvalidField(
                    "qualified resource reasons are not the declared gate result".into(),
                ));
            }
        }
        if self.omissions.windows(2).any(|pair| {
            (pair[0].resource_id.as_str(), pair[0].reason.as_str())
                >= (pair[1].resource_id.as_str(), pair[1].reason.as_str())
        }) {
            return Err(ResourceWorkbenchError::InvalidField(
                "resource omissions are not canonically ordered".into(),
            ));
        }
        let mut omission_ids = std::collections::BTreeSet::new();
        for omission in &self.omissions {
            validate_text("omission.resource_id", &omission.resource_id)?;
            validate_text("omission.reason", &omission.reason)?;
            if !omission_ids.insert(omission.resource_id.clone())
                || qualified_ids.contains(&omission.resource_id)
            {
                return Err(ResourceWorkbenchError::InvalidField(
                    "resource omissions must be disjoint and unique".into(),
                ));
            }
        }
        let expected_disposition = if self.qualified_resources.is_empty() {
            ResourceWorkbenchDisposition::Unknown
        } else if self.omissions.is_empty() {
            ResourceWorkbenchDisposition::Qualified
        } else {
            ResourceWorkbenchDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(ResourceWorkbenchError::InvalidField(
                "resource disposition does not match qualification and omission state".into(),
            ));
        }
        if self.checks != canonical_checks(self.disposition) {
            return Err(ResourceWorkbenchError::InvalidField(
                "resource workbench checks are not canonical".into(),
            ));
        }
        if self.artifact.artifact_id != format!("resource-workbench:{}", self.need_id)
            || self.artifact.content_type != "application/vnd.aurora.resource-workbench+json"
            || !self.artifact.semantic_loss.is_empty()
        {
            return Err(ResourceWorkbenchError::Artifact(
                "resource artifact is not bound to the qualification result".into(),
            ));
        }
        let expected_provenance = self
            .qualified_resources
            .iter()
            .map(|resource| ProvenanceLink {
                source_id: resource.resource_id.clone(),
                relation: "qualified-resource-artifact".into(),
                digest: resource.artifact_digest.clone(),
            })
            .collect::<Vec<_>>();
        if self.artifact.provenance != expected_provenance {
            return Err(ResourceWorkbenchError::Artifact(
                "resource artifact provenance is not bound to qualified resources".into(),
            ));
        }
        let payload = resource_payload(self);
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| ResourceWorkbenchError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| ResourceWorkbenchError::Artifact(error.to_string()))?;
        validate_request(
            &self.input.request_id,
            &self.input.need,
            &self.input.candidates,
        )?;
        if self.input_digest != resource_input_digest(&self.input)? {
            return Err(ResourceWorkbenchError::Artifact(
                "resource workbench retained input digest does not match the request".into(),
            ));
        }
        let expected = build_resource_discovery(&self.input)?;
        if self != &expected {
            return Err(ResourceWorkbenchError::Artifact(
                "resource workbench receipt is not derived from its retained request".into(),
            ));
        }
        Ok(())
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

fn validate_text(field: &str, value: &str) -> Result<(), ResourceWorkbenchError> {
    if value.is_empty() || value.trim() != value {
        return Err(ResourceWorkbenchError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ResourceWorkbenchError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn canonical_resource_discovery_request(
    request: &ResourceDiscoveryRequest,
) -> ResourceDiscoveryRequest {
    let mut canonical = request.clone();
    canonical.need.required_capabilities.sort();
    canonical.need.allowed_origins.sort();
    for candidate in &mut canonical.candidates {
        candidate.capabilities.sort();
    }
    canonical.candidates.sort_by(|left, right| {
        right
            .trust_score
            .total_cmp(&left.trust_score)
            .then_with(|| left.resource_id.cmp(&right.resource_id))
    });
    canonical
}

fn resource_input_digest(
    request: &ResourceDiscoveryRequest,
) -> Result<ContentHash, ResourceWorkbenchError> {
    let canonical = canonical_resource_discovery_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| ResourceWorkbenchError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| ResourceWorkbenchError::Serialization(error.to_string()))
}

fn validate_unique_strings(
    values: &[String],
    field: &str,
    max_items: usize,
) -> Result<(), ResourceWorkbenchError> {
    if values.len() > max_items {
        return Err(ResourceWorkbenchError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = std::collections::BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ResourceWorkbenchError::InvalidField(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(values: &[String], field: &str) -> Result<(), ResourceWorkbenchError> {
    validate_unique_strings(values, field, MAX_ITEMS)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ResourceWorkbenchError::InvalidField(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn canonical_checks(disposition: ResourceWorkbenchDisposition) -> Vec<String> {
    let mut checks = vec![
        "candidates are deterministically ranked by trust and resource identity".into(),
        "qualified resources retain content digests without moving raw bytes".into(),
        "stale, protected, out-of-scope, and non-local resources remain omissions".into(),
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
    checks.sort();
    checks
}

fn resource_payload(receipt: &ResourceWorkbenchReceipt) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": receipt.request_id,
        "need_id": receipt.need_id,
        "requester": receipt.requester,
        "intent": receipt.intent,
        "required_capabilities": receipt.required_capabilities,
        "allowed_origins": receipt.allowed_origins,
        "max_results": receipt.max_results,
        "federation_allowed": receipt.federation_allowed,
        "disposition": receipt.disposition,
        "qualified_resources": receipt.qualified_resources,
        "omissions": receipt.omissions,
        "checks": receipt.checks,
        "boundary": receipt.boundary,
    })
}

pub fn discover_resources(
    request_id: &str,
    need: &ResourceNeed,
    candidates: &[ResourceCandidate],
) -> Result<ResourceWorkbenchReceipt, ResourceWorkbenchError> {
    let request = ResourceDiscoveryRequest {
        request_id: request_id.into(),
        need: need.clone(),
        candidates: candidates.to_vec(),
    };
    let receipt = build_resource_discovery(&request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_resource_discovery(
    request: &ResourceDiscoveryRequest,
) -> Result<ResourceWorkbenchReceipt, ResourceWorkbenchError> {
    let canonical_request = canonical_resource_discovery_request(request);
    let request = &canonical_request;
    let request_id = &request.request_id;
    let need = &request.need;
    validate_request(request_id, need, &request.candidates)?;
    let mut candidates = request.candidates.clone();
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
                trust_score: candidate.trust_score,
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
    omissions.sort_by(|left, right| {
        (left.resource_id.as_str(), left.reason.as_str())
            .cmp(&(right.resource_id.as_str(), right.reason.as_str()))
    });
    let checks = canonical_checks(disposition);
    let input_digest = resource_input_digest(request)?;
    let mut receipt = ResourceWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        input: canonical_request.clone(),
        input_digest,
        request_id: request.request_id.clone(),
        need_id: need.need_id.clone(),
        requester: need.requester.clone(),
        intent: need.intent.clone(),
        required_capabilities: need.required_capabilities.clone(),
        allowed_origins: need.allowed_origins.clone(),
        max_results: need.max_results,
        federation_allowed: need.federation_allowed,
        disposition,
        qualified_resources: qualified,
        omissions,
        checks,
        artifact: TypedResearchArtifact {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            artifact_id: format!("resource-workbench:{}", need.need_id),
            content_type: "application/vnd.aurora.resource-workbench+json".into(),
            content_hash: ContentHash::of_bytes(b""),
            semantic_loss: Vec::new(),
            provenance: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = resource_payload(&receipt);
    let provenance = receipt
        .qualified_resources
        .iter()
        .map(|resource| ProvenanceLink {
            source_id: resource.resource_id.clone(),
            relation: "qualified-resource-artifact".into(),
            digest: resource.artifact_digest.clone(),
        })
        .collect::<Vec<_>>();
    receipt.artifact = TypedResearchArtifact::from_payload(
        format!("resource-workbench:{}", need.need_id),
        "application/vnd.aurora.resource-workbench+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| ResourceWorkbenchError::Artifact(error.to_string()))?;
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
    validate_text("request_id", request_id)?;
    validate_text("need_id", &need.need_id)?;
    validate_text("requester", &need.requester)?;
    validate_text("intent", &need.intent)?;
    if need.max_results == 0 || need.max_results > MAX_CANDIDATES {
        return Err(ResourceWorkbenchError::InvalidField(
            "resource result bound is outside its contract".into(),
        ));
    }
    validate_unique_strings(
        &need.required_capabilities,
        "required_capabilities",
        MAX_ITEMS,
    )?;
    validate_unique_strings(&need.allowed_origins, "allowed_origins", MAX_ITEMS)?;
    if candidates.len() > MAX_CANDIDATES {
        return Err(ResourceWorkbenchError::InvalidField(
            "resource candidates exceed their item bound".into(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for candidate in candidates {
        validate_text("resource_id", &candidate.resource_id)?;
        validate_text("resource.origin", &candidate.origin)?;
        if !ids.insert(candidate.resource_id.clone()) {
            return Err(ResourceWorkbenchError::InvalidField(
                "resource identities must be non-empty and unique".into(),
            ));
        }
        validate_unique_strings(&candidate.capabilities, "resource.capabilities", MAX_ITEMS)?;
        if candidate.artifact_digest == ContentHash::of_bytes(b"")
            || !candidate.trust_score.is_finite()
            || !(0.0..=1.0).contains(&candidate.trust_score)
        {
            return Err(ResourceWorkbenchError::InvalidField(format!(
                "resource {} has invalid trust or artifact identity",
                candidate.resource_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn need() -> ResourceNeed {
        ResourceNeed {
            need_id: "need:imaging".into(),
            requester: "researcher".into(),
            intent: "find local imaging".into(),
            required_capabilities: vec!["imaging".into()],
            allowed_origins: vec!["site:a".into()],
            max_results: 2,
            federation_allowed: false,
        }
    }

    fn candidate(resource_id: &str, trust_score: f64, protected: bool) -> ResourceCandidate {
        ResourceCandidate {
            resource_id: resource_id.into(),
            origin: "site:a".into(),
            capabilities: vec!["imaging".into()],
            artifact_digest: ContentHash::of_bytes(resource_id.as_bytes()),
            trust_score,
            available: true,
            protected,
            raw_data_local: true,
            federated: false,
        }
    }

    #[test]
    fn protected_resource_is_omitted() {
        let receipt = discover_resources(
            "request:resources",
            &need(),
            &[candidate("resource:protected", 0.9, true)],
        )
        .unwrap();
        assert_eq!(receipt.disposition, ResourceWorkbenchDisposition::Unknown);
        assert_eq!(
            receipt.omissions[0].reason,
            "resource is protected by institution policy"
        );
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn qualified_resources_retain_rankable_trust_metadata() {
        let receipt = discover_resources(
            "request:resources",
            &need(),
            &[
                candidate("resource:lower", 0.4, false),
                candidate("resource:higher", 0.9, false),
            ],
        )
        .unwrap();
        assert_eq!(receipt.disposition, ResourceWorkbenchDisposition::Qualified);
        assert_eq!(
            receipt.qualified_resources[0].resource_id,
            "resource:higher"
        );
        assert_eq!(receipt.qualified_resources[0].trust_score, 0.9);
        assert_eq!(receipt.qualified_resources[0].rank, 1);
    }

    #[test]
    fn invalid_candidate_trust_is_rejected_before_ranking() {
        let invalid = candidate("resource:invalid", f64::NAN, false);
        assert!(discover_resources("request:resources", &need(), &[invalid]).is_err());
    }

    #[test]
    fn resource_artifact_payload_is_verified() {
        let mut receipt = discover_resources(
            "request:resources",
            &need(),
            &[candidate("resource:ok", 0.9, false)],
        )
        .unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = discover_resources(
            "request:resources",
            &need(),
            &[candidate("resource:ok", 0.9, false)],
        )
        .unwrap();
        receipt.input.need.intent = "tampered intent".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn reordered_sets_and_candidates_have_stable_identity() {
        let mut reversed_need = need();
        reversed_need.required_capabilities.reverse();
        reversed_need.allowed_origins.reverse();
        let first = discover_resources(
            "request:resources",
            &need(),
            &[
                candidate("resource:lower", 0.4, false),
                candidate("resource:higher", 0.9, false),
            ],
        )
        .unwrap();
        let mut higher = candidate("resource:higher", 0.9, false);
        higher.capabilities.reverse();
        let mut lower = candidate("resource:lower", 0.4, false);
        lower.capabilities.reverse();
        let second =
            discover_resources("request:resources", &reversed_need, &[higher, lower]).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
}
