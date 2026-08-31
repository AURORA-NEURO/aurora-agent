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
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
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
const MAX_TEXT_BYTES: usize = 512;
const MAX_CAPABILITIES: usize = 4096;
const MAX_VERSIONS: usize = 32;
const MAX_ARTIFACT_DIGESTS: usize = 8192;
const MAX_RECEIPT_ITEMS: usize = 8192;

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
    pub capability_id: String,
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
        if !self.raw_data_local || self.boundary != PRECLINICAL_BOUNDARY {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "integration locality and boundary are required".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("capability_id", &self.capability_id)?;
        validate_text("endpoint_id", &self.endpoint_id)?;
        validate_text("negotiated_version", &self.negotiated_version)?;
        if self.replay_token == ContentHash::of_bytes(b"") {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "replay token is required".into(),
            ));
        }
        if self.capability_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "capabilities, checks, and effects are required".into(),
            ));
        }
        validate_sorted_strings(&self.capability_order, "capability_order", MAX_CAPABILITIES)?;
        validate_sorted_hashes(&self.artifact_digest_order, "artifact_digest_order")?;
        if self
            .artifact_digest_order
            .iter()
            .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "artifact digest order cannot contain empty digests".into(),
            ));
        }
        validate_sorted_strings(&self.omissions, "omissions", MAX_RECEIPT_ITEMS)?;
        validate_sorted_strings(&self.uncertainty, "uncertainty", MAX_RECEIPT_ITEMS)?;
        validate_sorted_strings(&self.checks, "checks", MAX_RECEIPT_ITEMS)?;
        validate_sorted_strings(&self.effect_receipts, "effect_receipts", MAX_RECEIPT_ITEMS)?;
        if self.semantic_loss.len() > MAX_RECEIPT_ITEMS {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "semantic_loss exceeds its item bound".into(),
            ));
        }
        for loss in &self.semantic_loss {
            validate_text("semantic_loss.field", &loss.field)?;
            validate_text("semantic_loss.reason", &loss.reason)?;
        }
        if self.semantic_loss.windows(2).any(|pair| {
            (
                pair[0].field.as_str(),
                pair[0].reason.as_str(),
                pair[0].severity,
            ) >= (
                pair[1].field.as_str(),
                pair[1].reason.as_str(),
                pair[1].severity,
            )
        }) {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "semantic_loss ordering is not canonical".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:")
                && !effect.starts_with("blocked:")
                && !effect.starts_with("approval-required:")
        }) {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "interoperability effect is outside the exchange gate".into(),
            ));
        }
        let has_exchange = self
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("exchange:"));
        match self.disposition {
            InteroperabilityDisposition::Accepted | InteroperabilityDisposition::Migrated
                if !has_exchange || self.effect_receipts.len() != 1 =>
            {
                return Err(InteroperabilityGatewayError::InvalidRequest(
                    "accepted integrations require exactly one exchange effect".into(),
                ));
            }
            InteroperabilityDisposition::Incompatible
                if has_exchange
                    || self.effect_receipts
                        != vec!["blocked:incompatible-contract".to_string()] =>
            {
                return Err(InteroperabilityGatewayError::InvalidRequest(
                    "incompatible integrations must be blocked without exchange".into(),
                ));
            }
            InteroperabilityDisposition::Blocked
                if self.effect_receipts
                    != vec!["blocked:no-permitted-artifact-exchange".to_string()] =>
            {
                return Err(InteroperabilityGatewayError::InvalidRequest(
                    "blocked integrations require an authorization block".into(),
                ));
            }
            InteroperabilityDisposition::ApprovalRequired
                if self.effect_receipts
                    != vec!["approval-required:protected-closure".to_string()] =>
            {
                return Err(InteroperabilityGatewayError::InvalidRequest(
                    "approval-required integrations require a closure gate".into(),
                ));
            }
            InteroperabilityDisposition::Unknown
                if self.effect_receipts != vec!["blocked:missing-capabilities".to_string()] =>
            {
                return Err(InteroperabilityGatewayError::InvalidRequest(
                    "unknown integrations require a capability block".into(),
                ));
            }
            _ => {}
        }
        if matches!(self.disposition, InteroperabilityDisposition::Accepted)
            && (!self.semantic_loss.is_empty()
                || !self.omissions.is_empty()
                || !self.uncertainty.is_empty())
        {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "accepted integrations cannot contain unresolved loss or uncertainty".into(),
            ));
        }
        if matches!(self.disposition, InteroperabilityDisposition::Migrated)
            && self.semantic_loss.is_empty()
        {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "migrated integrations require semantic loss".into(),
            ));
        }
        if self.checks != canonical_checks(&self.semantic_loss) {
            return Err(InteroperabilityGatewayError::InvalidRequest(
                "integration checks are not bound to the negotiated migration state".into(),
            ));
        }
        if self.artifact.artifact_id != format!("interoperability-gateway:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.negotiated-integration+json"
            || self.artifact.semantic_loss != self.semantic_loss
        {
            return Err(InteroperabilityGatewayError::Contract(
                "integration artifact is not bound to the receipt".into(),
            ));
        }
        let expected_provenance =
            integration_provenance(&self.endpoint_id, &self.artifact_digest_order);
        if self.artifact.provenance != expected_provenance {
            return Err(InteroperabilityGatewayError::Contract(
                "integration artifact provenance is not bound to advertised digests".into(),
            ));
        }
        let payload = negotiated_payload(
            &self.request_id,
            &self.capability_id,
            &self.endpoint_id,
            &self.negotiated_version,
            self.disposition,
            &self.capability_order,
            &self.artifact_digest_order,
            &self.replay_token,
            &self.omissions,
            &self.uncertainty,
            &self.semantic_loss,
            &self.checks,
            &self.effect_receipts,
            self.raw_data_local,
            &self.boundary,
        );
        self.artifact
            .verify_payload(&payload)
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

fn validate_text(field: &str, value: &str) -> Result<(), InteroperabilityGatewayError> {
    if value.is_empty() || value.trim() != value {
        return Err(InteroperabilityGatewayError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(InteroperabilityGatewayError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_input_strings(
    values: &[String],
    field: &str,
    max_items: usize,
) -> Result<(), InteroperabilityGatewayError> {
    if values.len() > max_items {
        return Err(InteroperabilityGatewayError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(InteroperabilityGatewayError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    values: &[String],
    field: &str,
    max_items: usize,
) -> Result<(), InteroperabilityGatewayError> {
    validate_input_strings(values, field, max_items)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(InteroperabilityGatewayError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_sorted_hashes(
    values: &[ContentHash],
    field: &str,
) -> Result<(), InteroperabilityGatewayError> {
    if values.len() > MAX_ARTIFACT_DIGESTS || values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(InteroperabilityGatewayError::InvalidRequest(format!(
            "{field} ordering or size is not canonical"
        )));
    }
    Ok(())
}

fn integration_provenance(
    endpoint_id: &str,
    artifact_digests: &[ContentHash],
) -> Vec<ProvenanceLink> {
    artifact_digests
        .iter()
        .cloned()
        .map(|digest| ProvenanceLink {
            source_id: endpoint_id.into(),
            relation: "advertised-artifact-digest".into(),
            digest,
        })
        .collect()
}

fn canonical_checks(semantic_loss: &[SemanticLoss]) -> Vec<String> {
    let mut checks = vec![
        "capability names canonicalized and compared as a set".to_string(),
        "raw artifact bytes remain institution-local".to_string(),
    ];
    if !semantic_loss.is_empty() {
        checks.push("compatible source version requires explicit migration receipt".into());
    }
    checks.sort();
    checks
}

#[allow(clippy::too_many_arguments)]
fn negotiated_payload(
    request_id: &str,
    capability_id: &str,
    endpoint_id: &str,
    negotiated_version: &str,
    disposition: InteroperabilityDisposition,
    capability_order: &[String],
    artifact_digest_order: &[ContentHash],
    replay_token: &ContentHash,
    omissions: &[String],
    uncertainty: &[String],
    semantic_loss: &[SemanticLoss],
    checks: &[String],
    effect_receipts: &[String],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request_id,
        "capability_id": capability_id,
        "endpoint_id": endpoint_id,
        "negotiated_version": negotiated_version,
        "disposition": disposition,
        "capability_order": capability_order,
        "artifact_digest_order": artifact_digest_order,
        "replay_token": replay_token,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "checks": checks,
        "effect_receipts": effect_receipts,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut semantic_loss = Vec::new();
    let mut effect_receipts = Vec::new();
    let missing_from_source = request
        .target_capabilities
        .iter()
        .filter(|capability| !source.offered_capabilities.contains(capability))
        .cloned()
        .collect::<Vec<_>>();
    let missing_from_target = source
        .required_capabilities
        .iter()
        .filter(|capability| !request.target_capabilities.contains(capability))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_from_source.is_empty() || !missing_from_target.is_empty() {
        let mut missing = missing_from_source.clone();
        missing.extend(
            missing_from_target
                .iter()
                .map(|value| format!("target:{value}")),
        );
        missing.sort();
        omissions.push(format!(
            "target capabilities unavailable: {}",
            missing.join(",")
        ));
        uncertainty
            .push("the integration cannot establish parity for unavailable capabilities".into());
    }
    let source_advertises_target = source
        .supported_contract_versions
        .iter()
        .any(|version| version == &request.target_contract_version);
    let (negotiated_version, version_disposition) = if !source_advertises_target {
        uncertainty
            .push("source did not advertise support for the requested contract version".into());
        (
            request.target_contract_version.clone(),
            InteroperabilityDisposition::Incompatible,
        )
    } else if request.target_contract_version == TARGET_CONTRACT_VERSION
        && source.source_contract_version == TARGET_CONTRACT_VERSION
    {
        (
            TARGET_CONTRACT_VERSION.to_string(),
            InteroperabilityDisposition::Accepted,
        )
    } else if request.target_contract_version == TARGET_CONTRACT_VERSION
        && source.source_contract_version == COMPATIBLE_CONTRACT_VERSION
    {
        omissions.push("legacy fields outside the pinned target contract remain unknown".into());
        semantic_loss.push(SemanticLoss {
            field: "legacy_fields".into(),
            reason: "additive migration cannot infer omitted semantics".into(),
            severity: LossSeverity::Unknown,
        });
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
    } else if !missing_from_source.is_empty() || !missing_from_target.is_empty() {
        effect_receipts.push("blocked:missing-capabilities".into());
        InteroperabilityDisposition::Unknown
    } else if matches!(
        version_disposition,
        InteroperabilityDisposition::Incompatible
    ) {
        effect_receipts.push("blocked:incompatible-contract".into());
        InteroperabilityDisposition::Incompatible
    } else {
        effect_receipts.push("exchange:permitted-artifact-digests-only".into());
        version_disposition
    };
    omissions.sort();
    uncertainty.sort();
    semantic_loss.sort_by(|left, right| {
        (left.field.as_str(), left.reason.as_str(), left.severity).cmp(&(
            right.field.as_str(),
            right.reason.as_str(),
            right.severity,
        ))
    });
    let checks = canonical_checks(&semantic_loss);
    effect_receipts.sort();
    let payload = negotiated_payload(
        &request.request_id,
        &source.capability_id,
        &source.endpoint_id,
        &negotiated_version,
        disposition,
        &capability_order,
        &artifact_digest_order,
        &request.replay_token,
        &omissions,
        &uncertainty,
        &semantic_loss,
        &checks,
        &effect_receipts,
        request.raw_data_local,
        &request.boundary,
    );
    let provenance = integration_provenance(&source.endpoint_id, &artifact_digest_order);
    let artifact = TypedResearchArtifact::from_payload(
        format!("interoperability-gateway:{}", request.request_id),
        "application/vnd.aurora.negotiated-integration+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| InteroperabilityGatewayError::Contract(error.to_string()))?;
    let result = NegotiatedIntegration {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        capability_id: source.capability_id.clone(),
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
    if !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !source.raw_data_local
        || source.boundary != PRECLINICAL_BOUNDARY
        || request.target_capabilities.is_empty()
        || source.supported_contract_versions.is_empty()
        || source.offered_capabilities.is_empty()
        || source.artifact_digests.is_empty()
    {
        return Err(InteroperabilityGatewayError::InvalidRequest(
            "request capabilities, artifact digests, locality, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("target_contract_version", &request.target_contract_version)?;
    validate_text("capability_id", &source.capability_id)?;
    validate_text("endpoint_id", &source.endpoint_id)?;
    validate_text("source_contract_version", &source.source_contract_version)?;
    validate_text("boundary", &request.boundary)?;
    validate_text("source.boundary", &source.boundary)?;
    if request.replay_token == ContentHash::of_bytes(b"") {
        return Err(InteroperabilityGatewayError::InvalidRequest(
            "replay token is required".into(),
        ));
    }
    validate_input_strings(
        &request.target_capabilities,
        "target_capabilities",
        MAX_CAPABILITIES,
    )?;
    validate_input_strings(
        &source.offered_capabilities,
        "offered_capabilities",
        MAX_CAPABILITIES,
    )?;
    validate_input_strings(
        &source.required_capabilities,
        "required_capabilities",
        MAX_CAPABILITIES,
    )?;
    validate_input_strings(
        &source.supported_contract_versions,
        "supported_contract_versions",
        MAX_VERSIONS,
    )?;
    if !source
        .supported_contract_versions
        .contains(&source.source_contract_version)
    {
        return Err(InteroperabilityGatewayError::InvalidRequest(
            "source contract version must be included in its advertised versions".into(),
        ));
    }
    if source
        .artifact_digests
        .windows(2)
        .any(|pair| pair[0] == pair[1])
        || {
            let mut digests = source.artifact_digests.clone();
            let original_len = digests.len();
            digests.sort();
            digests.dedup();
            original_len != digests.len()
        }
    {
        return Err(InteroperabilityGatewayError::InvalidRequest(
            "artifact_digests cannot contain duplicates".into(),
        ));
    }
    if source.artifact_digests.len() > MAX_ARTIFACT_DIGESTS {
        return Err(InteroperabilityGatewayError::InvalidRequest(
            "artifact_digests exceeds its item bound".into(),
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

    #[test]
    fn source_required_capability_must_be_provided_by_target() {
        let mut request = request();
        request
            .source
            .required_capabilities
            .push("target-qc".into());
        let result = negotiate_interoperability(&request).unwrap();
        assert_eq!(result.disposition, InteroperabilityDisposition::Unknown);
        assert!(result
            .omissions
            .iter()
            .any(|omission| omission.contains("target:target-qc")));
    }

    #[test]
    fn source_must_advertise_the_requested_version() {
        let mut request = request();
        request.source.source_contract_version = COMPATIBLE_CONTRACT_VERSION.into();
        request.source.supported_contract_versions = vec![COMPATIBLE_CONTRACT_VERSION.into()];
        let result = negotiate_interoperability(&request).unwrap();
        assert_eq!(
            result.disposition,
            InteroperabilityDisposition::Incompatible
        );
        assert_eq!(
            result.effect_receipts,
            vec!["blocked:incompatible-contract"]
        );
    }

    #[test]
    fn empty_replay_token_is_rejected() {
        let mut request = request();
        request.replay_token = ContentHash::of_bytes(b"");
        assert!(negotiate_interoperability(&request).is_err());
    }

    #[test]
    fn duplicate_artifact_digest_is_rejected() {
        let mut request = request();
        let digest = request.source.artifact_digests[0].clone();
        request.source.artifact_digests.push(digest);
        assert!(negotiate_interoperability(&request).is_err());
    }

    #[test]
    fn receipt_cannot_add_exchange_to_a_blocked_outcome() {
        let mut receipt = negotiate_interoperability(&request()).unwrap();
        receipt.effect_receipts = vec![
            "blocked:no-permitted-artifact-exchange".into(),
            "exchange:permitted-artifact-digests-only".into(),
        ];
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn source_version_must_be_self_advertised() {
        let mut request = request();
        request.source.source_contract_version = COMPATIBLE_CONTRACT_VERSION.into();
        assert!(negotiate_interoperability(&request).is_err());
    }

    #[test]
    fn negotiated_artifact_payload_is_verified() {
        let mut receipt = negotiate_interoperability(&request()).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn artifact_provenance_is_bound_to_advertised_digests() {
        let mut receipt = negotiate_interoperability(&request()).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }
}
