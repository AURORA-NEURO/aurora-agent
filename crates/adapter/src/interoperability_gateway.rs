//! Federated continual interoperability gateway.
//!
//! Atlas feature: `AFA-adapter-P22-F24`.
//!
//! This gateway negotiates a version-pinned capability contract across institution-local
//! endpoints. It exchanges only explicitly permitted artifact digests and capability metadata;
//! it never fetches raw data, executes a remote tool, or treats protocol compatibility as a
//! scientific conclusion. Unsupported versions, ambiguous capability projections, missing
//! protected closure, and denied export fail closed with replayable loss receipts.

use bioprism_foundation::{
    LossSeverity, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P22-F24";
pub const CONTRACT_VERSION: &str = "federated-interoperability-gateway/1.0";
pub const TARGET_CONTRACT_VERSION: &str = "1.0.0";
pub const COMPATIBLE_CONTRACT_VERSION: &str = "0.9.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCapability {
    pub capability_id: String,
    pub endpoint_id: String,
    pub source_contract_version: String,
    pub supported_contract_versions: Vec<String>,
    pub offered_capabilities: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub artifact_digests: Vec<ContentHash>,
    pub permitted_export: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteroperabilityRequest {
    pub request_id: String,
    pub source: ExternalCapability,
    pub target_contract_version: String,
    pub target_capabilities: Vec<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub replay_token: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteroperabilityDisposition {
    Accepted,
    Migrated,
    ApprovalRequired,
    Blocked,
    Incompatible,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedIntegration {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub endpoint_id: String,
    pub negotiated_version: String,
    pub disposition: InteroperabilityDisposition,
    pub capability_order: Vec<String>,
    pub artifact_digest_order: Vec<ContentHash>,
    pub replay_token: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub checks: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl NegotiatedIntegration {
    pub fn validate(&self) -> Result<(), InteroperabilityGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(InteroperabilityGatewayError::Contract(
                "interoperability identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.endpoint_id.trim().is_empty()
            || self.negotiated_version.trim().is_empty()
            || self.capability_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(InteroperabilityGatewayError::InvalidRequest("integration identity, capabilities, checks, effects, locality, and boundary are required".into()));
        }
        if self
            .capability_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || self
                .artifact_digest_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "interoperability output ordering is not canonical".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InteroperabilityGatewayError::Contract(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, InteroperabilityGatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| InteroperabilityGatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| InteroperabilityGatewayError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum InteroperabilityGatewayError {
    #[error("invalid interoperability request: {0}")]
    InvalidRequest(String),
    #[error("interoperability gateway contract rejected: {0}")]
    Contract(String),
    #[error("interoperability gateway serialization failed: {0}")]
    Serialization(String),
}

pub fn negotiate_interoperability(
    request: &InteroperabilityRequest,
) -> Result<NegotiatedIntegration, InteroperabilityGatewayError> {
    validate_request(request)?;
    let source = &request.source;
    let mut capability_order = source
        .offered_capabilities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    capability_order.extend(source.required_capabilities.iter().cloned());
    capability_order.extend(request.target_capabilities.iter().cloned());
    let capability_order = capability_order.into_iter().collect::<Vec<_>>();
    let mut artifact_digest_order = source.artifact_digests.clone();
    artifact_digest_order.sort();
    artifact_digest_order.dedup();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut semantic_loss = Vec::new();
    let mut checks = vec![
        "capability names canonicalized and compared as a set".into(),
        "raw artifact bytes remain institution-local".into(),
    ];
    let mut effect_receipts = Vec::new();
    let missing = request
        .target_capabilities
        .iter()
        .filter(|capability| !source.offered_capabilities.contains(capability))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        omissions.push(format!(
            "target capabilities unavailable: {}",
            missing.join(",")
        ));
        uncertainty
            .push("the integration cannot establish parity for unavailable capabilities".into());
    }
    let (negotiated_version, version_disposition) = if request.target_contract_version
        == TARGET_CONTRACT_VERSION
        && source.source_contract_version == TARGET_CONTRACT_VERSION
    {
        (
            TARGET_CONTRACT_VERSION.to_string(),
            InteroperabilityDisposition::Accepted,
        )
    } else if request.target_contract_version == TARGET_CONTRACT_VERSION
        && source.source_contract_version == COMPATIBLE_CONTRACT_VERSION
        && source
            .supported_contract_versions
            .iter()
            .any(|version| version == TARGET_CONTRACT_VERSION)
    {
        omissions.push("legacy fields outside the pinned target contract remain unknown".into());
        semantic_loss.push(SemanticLoss {
            field: "legacy_fields".into(),
            reason: "additive migration cannot infer omitted semantics".into(),
            severity: LossSeverity::Unknown,
        });
        checks.push("compatible source version requires explicit migration receipt".into());
        (
            TARGET_CONTRACT_VERSION.to_string(),
            InteroperabilityDisposition::Migrated,
        )
    } else {
        uncertainty.push(
            "source and target contract versions are outside the pinned compatibility window"
                .into(),
        );
        (
            request.target_contract_version.clone(),
            InteroperabilityDisposition::Incompatible,
        )
    };
    let disposition = if !request.policy_allow || !source.permitted_export {
        omissions.push("policy or endpoint authorization denied artifact exchange".into());
        effect_receipts.push("blocked:no-permitted-artifact-exchange".into());
        InteroperabilityDisposition::Blocked
    } else if !request.protected_closure {
        omissions.push("protected closure is incomplete".into());
        uncertainty
            .push("compatibility cannot be promoted while required evidence is unmeasured".into());
        effect_receipts.push("approval-required:protected-closure".into());
        InteroperabilityDisposition::ApprovalRequired
    } else if !missing.is_empty() {
        effect_receipts.push("blocked:missing-capabilities".into());
        InteroperabilityDisposition::Unknown
    } else {
        effect_receipts.push("exchange:permitted-artifact-digests-only".into());
        version_disposition
    };
    if matches!(disposition, InteroperabilityDisposition::Incompatible) {
        effect_receipts.push("blocked:incompatible-contract".into());
    }
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "endpoint_id": source.endpoint_id,
        "negotiated_version": negotiated_version,
        "disposition": disposition,
        "capability_order": capability_order,
        "artifact_digest_order": artifact_digest_order,
        "replay_token": request.replay_token,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "checks": checks,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("interoperability-gateway:{}", request.request_id),
        "application/vnd.aurora.negotiated-integration+json",
        &payload,
        semantic_loss.clone(),
        Vec::new(),
    )
    .map_err(|error| InteroperabilityGatewayError::Contract(error.to_string()))?;
    let result = NegotiatedIntegration {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        endpoint_id: source.endpoint_id.clone(),
        negotiated_version,
        disposition,
        capability_order,
        artifact_digest_order,
        replay_token: request.replay_token.clone(),
        omissions,
        uncertainty,
        semantic_loss,
        checks,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    result.validate()?;
    Ok(result)
}

fn validate_request(request: &InteroperabilityRequest) -> Result<(), InteroperabilityGatewayError> {
    let source = &request.source;
    if request.request_id.trim().is_empty()
        || request.target_contract_version.trim().is_empty()
        || request.target_capabilities.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !source.raw_data_local
        || source.boundary != PRECLINICAL_BOUNDARY
        || source.capability_id.trim().is_empty()
        || source.endpoint_id.trim().is_empty()
        || source.source_contract_version.trim().is_empty()
        || source.supported_contract_versions.is_empty()
        || source.offered_capabilities.is_empty()
        || source.artifact_digests.is_empty()
    {
        return Err(InteroperabilityGatewayError::InvalidRequest("request, endpoint, versions, capabilities, artifact digests, locality, authorization, and boundary are required".into()));
    }
    if request
        .target_capabilities
        .iter()
        .any(|value| value.trim().is_empty())
        || source
            .offered_capabilities
            .iter()
            .any(|value| value.trim().is_empty())
        || source
            .required_capabilities
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(InteroperabilityGatewayError::InvalidRequest(
            "capability names cannot be empty".into(),
        ));
    }
    if source
        .supported_contract_versions
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(InteroperabilityGatewayError::InvalidRequest(
            "supported contract versions cannot be empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> InteroperabilityRequest {
        InteroperabilityRequest {
            request_id: "request:qc".into(),
            source: ExternalCapability {
                capability_id: "capability:qc".into(),
                endpoint_id: "endpoint:site-a".into(),
                source_contract_version: TARGET_CONTRACT_VERSION.into(),
                supported_contract_versions: vec![TARGET_CONTRACT_VERSION.into()],
                offered_capabilities: vec!["artifact-digest".into(), "qc-summary".into()],
                required_capabilities: vec!["artifact-digest".into()],
                artifact_digests: vec![ContentHash::of_bytes(b"qc")],
                permitted_export: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            target_contract_version: TARGET_CONTRACT_VERSION.into(),
            target_capabilities: vec!["artifact-digest".into(), "qc-summary".into()],
            policy_allow: true,
            protected_closure: true,
            replay_token: ContentHash::of_bytes(b"replay"),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn gateway_is_deterministic_and_digest_only() {
        let first = negotiate_interoperability(&request()).unwrap();
        let second = negotiate_interoperability(&request()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.disposition, InteroperabilityDisposition::Accepted);
        assert!(first
            .effect_receipts
            .iter()
            .all(|receipt| receipt.contains("digests")));
    }
    #[test]
    fn compatible_version_retains_loss_receipt() {
        let mut request = request();
        request.source.source_contract_version = COMPATIBLE_CONTRACT_VERSION.into();
        request.source.supported_contract_versions = vec![
            COMPATIBLE_CONTRACT_VERSION.into(),
            TARGET_CONTRACT_VERSION.into(),
        ];
        let result = negotiate_interoperability(&request).unwrap();
        assert_eq!(result.disposition, InteroperabilityDisposition::Migrated);
        assert!(!result.semantic_loss.is_empty());
    }
    #[test]
    fn missing_capability_is_unknown_not_pass() {
        let mut request = request();
        request
            .target_capabilities
            .push("missing-capability".into());
        let result = negotiate_interoperability(&request).unwrap();
        assert_eq!(result.disposition, InteroperabilityDisposition::Unknown);
        assert!(!result.omissions.is_empty());
    }
    #[test]
    fn incomplete_closure_requires_approval() {
        let mut request = request();
        request.protected_closure = false;
        let result = negotiate_interoperability(&request).unwrap();
        assert_eq!(
            result.disposition,
            InteroperabilityDisposition::ApprovalRequired
        );
    }
    #[test]
    fn denied_export_blocks_exchange() {
        let mut request = request();
        request.source.permitted_export = false;
        let result = negotiate_interoperability(&request).unwrap();
        assert_eq!(result.disposition, InteroperabilityDisposition::Blocked);
    }
}
