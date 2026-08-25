//! Federated typed-determinism interoperability gateway.
//!
//! Atlas feature: `AFA-adapter-P17-F24`.
//!
//! The gateway exchanges only canonical metadata and content hashes. It negotiates a pinned
//! contract version, preserves unknown fields as omissions, and refuses endpoint or permission
//! ambiguity rather than manufacturing cross-language parity.

use bioprism_foundation::{
    LossSeverity, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P17-F24";
pub const CONTRACT_VERSION: &str = "typed-determinism-gateway/1.0";
pub const CURRENT_INPUT_VERSION: &str = "1.0.0";
pub const COMPATIBLE_INPUT_VERSION: &str = "0.9.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedCapabilityInput {
    pub capability_id: String,
    pub source_contract_version: String,
    pub endpoint_id: String,
    pub values: BTreeMap<String, serde_json::Value>,
    pub permitted_export: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterminismGatewayVerdict {
    Accepted,
    Migrated,
    ApprovalRequired,
    Incompatible,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalCapabilityOutput {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub capability_id: String,
    pub endpoint_id: String,
    pub negotiated_version: String,
    pub verdict: DeterminismGatewayVerdict,
    pub canonical_field_order: Vec<String>,
    pub canonical_input_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub effect_receipt: String,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl CanonicalCapabilityOutput {
    pub fn validate(&self) -> Result<(), DeterminismGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(DeterminismGatewayError::Contract(
                "typed determinism identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.capability_id.trim().is_empty()
            || self.endpoint_id.trim().is_empty()
            || self.canonical_field_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipt.trim().is_empty()
        {
            return Err(DeterminismGatewayError::InvalidRequest(
                "gateway identity, fields, reasons, locality, effects, and boundary are required"
                    .into(),
            ));
        }
        if self
            .canonical_field_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(DeterminismGatewayError::InvalidRequest(
                "canonical field order is not strictly sorted".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| DeterminismGatewayError::Contract(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, DeterminismGatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| DeterminismGatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| DeterminismGatewayError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum DeterminismGatewayError {
    #[error("invalid typed determinism request: {0}")]
    InvalidRequest(String),
    #[error("typed determinism contract rejected: {0}")]
    Contract(String),
    #[error("typed determinism serialization failed: {0}")]
    Serialization(String),
}

pub fn negotiate_capability(
    input: &TypedCapabilityInput,
) -> Result<CanonicalCapabilityOutput, DeterminismGatewayError> {
    validate_input(input)?;
    let field_order = input.values.keys().cloned().collect::<Vec<_>>();
    let canonical_values = serde_json::to_value(&input.values)
        .map_err(|error| DeterminismGatewayError::Serialization(error.to_string()))?;
    let input_digest = ContentHash::of_value(&canonical_values)
        .map_err(|error| DeterminismGatewayError::Serialization(error.to_string()))?;
    let (negotiated_version, verdict) = match input.source_contract_version.as_str() {
        CURRENT_INPUT_VERSION => (
            CURRENT_INPUT_VERSION.to_string(),
            if input.permitted_export {
                DeterminismGatewayVerdict::Accepted
            } else {
                DeterminismGatewayVerdict::Blocked
            },
        ),
        COMPATIBLE_INPUT_VERSION => (
            CURRENT_INPUT_VERSION.to_string(),
            if input.permitted_export {
                DeterminismGatewayVerdict::Migrated
            } else {
                DeterminismGatewayVerdict::Blocked
            },
        ),
        _ => (
            input.source_contract_version.clone(),
            DeterminismGatewayVerdict::Incompatible,
        ),
    };
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut semantic_loss = Vec::new();
    let mut reasons = vec![format!(
        "{} typed fields canonicalized with byte-stable key ordering",
        field_order.len()
    )];
    if input.source_contract_version == COMPATIBLE_INPUT_VERSION {
        omissions.push("legacy fields outside the pinned 1.0.0 contract remain unknown".into());
        semantic_loss.push(SemanticLoss {
            field: "legacy_fields".into(),
            reason: "additive migration cannot infer omitted semantics".into(),
            severity: LossSeverity::Unknown,
        });
        reasons.push("compatible migration retained unknown legacy fields as omissions".into());
    }
    if !input.permitted_export {
        reasons.push("endpoint policy did not permit artifact exchange".into());
        uncertainty.push("policy denial is not evidence about capability validity".into());
    }
    if verdict == DeterminismGatewayVerdict::Incompatible {
        reasons
            .push("source contract version is outside the negotiated compatibility window".into());
        uncertainty.push("canonical parity is unknown for an unsupported version".into());
    }
    let effect_receipt = if matches!(
        verdict,
        DeterminismGatewayVerdict::Accepted | DeterminismGatewayVerdict::Migrated
    ) {
        "exchange:permitted-artifacts"
    } else {
        "block:unauthorized-or-incompatible-exchange"
    };
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "capability_id": input.capability_id, "endpoint_id": input.endpoint_id, "negotiated_version": negotiated_version, "verdict": verdict, "canonical_field_order": field_order, "canonical_input_digest": input_digest, "omissions": omissions, "uncertainty": uncertainty, "semantic_loss": semantic_loss, "reasons": reasons, "effect_receipt": effect_receipt, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("typed-capability:{}", input.capability_id),
        "application/vnd.aurora.canonical-capability-output+json",
        &payload,
        semantic_loss.clone(),
        Vec::new(),
    )
    .map_err(|error| DeterminismGatewayError::Contract(error.to_string()))?;
    let output = CanonicalCapabilityOutput {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        capability_id: input.capability_id.clone(),
        endpoint_id: input.endpoint_id.clone(),
        negotiated_version,
        verdict,
        canonical_field_order: field_order,
        canonical_input_digest: input_digest,
        omissions,
        uncertainty,
        semantic_loss,
        reasons,
        effect_receipt: effect_receipt.into(),
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    output.validate()?;
    Ok(output)
}

fn validate_input(input: &TypedCapabilityInput) -> Result<(), DeterminismGatewayError> {
    if input.capability_id.trim().is_empty()
        || input.endpoint_id.trim().is_empty()
        || input.source_contract_version.trim().is_empty()
        || input.values.is_empty()
        || !input.raw_data_local
        || input.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(DeterminismGatewayError::InvalidRequest(
            "capability, source version, endpoint, values, locality, and boundary are required"
                .into(),
        ));
    }
    if input.values.keys().any(|key| key.trim().is_empty()) {
        return Err(DeterminismGatewayError::InvalidRequest(
            "capability field names cannot be empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> TypedCapabilityInput {
        TypedCapabilityInput {
            capability_id: "capability:qc".into(),
            source_contract_version: CURRENT_INPUT_VERSION.into(),
            endpoint_id: "endpoint:site-a".into(),
            values: [
                ("algorithm", json!("qc-v2")),
                ("schema", json!("1.0")),
                ("threshold", json!(0.95)),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
            permitted_export: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn canonical_gateway_is_deterministic() {
        let first = negotiate_capability(&input()).unwrap();
        let second = negotiate_capability(&input()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.verdict, DeterminismGatewayVerdict::Accepted);
    }
    #[test]
    fn legacy_version_is_migrated_with_omission() {
        let mut input = input();
        input.source_contract_version = COMPATIBLE_INPUT_VERSION.into();
        let output = negotiate_capability(&input).unwrap();
        assert_eq!(output.verdict, DeterminismGatewayVerdict::Migrated);
        assert!(!output.omissions.is_empty());
    }
    #[test]
    fn unsupported_version_is_incompatible() {
        let mut input = input();
        input.source_contract_version = "9.0.0".into();
        let output = negotiate_capability(&input).unwrap();
        assert_eq!(output.verdict, DeterminismGatewayVerdict::Incompatible);
    }
    #[test]
    fn denied_endpoint_blocks_exchange() {
        let mut input = input();
        input.permitted_export = false;
        let output = negotiate_capability(&input).unwrap();
        assert_eq!(output.verdict, DeterminismGatewayVerdict::Blocked);
    }
}
