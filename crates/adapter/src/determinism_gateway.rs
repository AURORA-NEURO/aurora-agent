//! Federated typed-determinism interoperability gateway.
//!
//! Atlas feature: `AFA-adapter-P17-F24`.
//!
//! The gateway exchanges only canonical metadata and content hashes. It negotiates a pinned
//! contract version, preserves unknown fields as omissions, and refuses endpoint or permission
//! ambiguity rather than manufacturing cross-language parity.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
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
const MAX_TEXT_BYTES: usize = 512;
const MAX_FIELDS: usize = 16384;
const MAX_VALUE_BYTES: usize = 1_048_576;

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
    pub source_contract_version: String,
    pub values: BTreeMap<String, serde_json::Value>,
    pub permitted_export: bool,
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
        validate_text("capability_id", &self.capability_id)?;
        validate_text("endpoint_id", &self.endpoint_id)?;
        validate_text("source_contract_version", &self.source_contract_version)?;
        validate_text("negotiated_version", &self.negotiated_version)?;
        validate_text("effect_receipt", &self.effect_receipt)?;
        if self.values.len() > MAX_FIELDS || self.values.is_empty() {
            return Err(DeterminismGatewayError::InvalidRequest(
                "gateway values are missing or exceed their field bound".into(),
            ));
        }
        let encoded_values = serde_json::to_vec(&self.values)
            .map_err(|error| DeterminismGatewayError::Serialization(error.to_string()))?;
        if encoded_values.len() > MAX_VALUE_BYTES {
            return Err(DeterminismGatewayError::InvalidRequest(
                "canonical capability values exceed their size bound".into(),
            ));
        }
        let values = serde_json::to_value(&self.values)
            .map_err(|error| DeterminismGatewayError::Serialization(error.to_string()))?;
        let expected_input_digest = ContentHash::of_value(&values)
            .map_err(|error| DeterminismGatewayError::Serialization(error.to_string()))?;
        if self.canonical_input_digest != expected_input_digest
            || self.canonical_field_order != self.values.keys().cloned().collect::<Vec<_>>()
        {
            return Err(DeterminismGatewayError::InvalidRequest(
                "canonical capability fields or digest are not derived from retained values".into(),
            ));
        }
        let input = TypedCapabilityInput {
            capability_id: self.capability_id.clone(),
            source_contract_version: self.source_contract_version.clone(),
            endpoint_id: self.endpoint_id.clone(),
            values: self.values.clone(),
            permitted_export: self.permitted_export,
            raw_data_local: self.raw_data_local,
            boundary: self.boundary.clone(),
        };
        validate_input(&input)?;
        let expected_effect = if matches!(
            self.verdict,
            DeterminismGatewayVerdict::Accepted | DeterminismGatewayVerdict::Migrated
        ) {
            "exchange:permitted-artifacts"
        } else {
            "block:unauthorized-or-incompatible-exchange"
        };
        if self.effect_receipt != expected_effect {
            return Err(DeterminismGatewayError::InvalidRequest(
                "effect receipt does not match the determinism verdict".into(),
            ));
        }
        let derived = derive_gateway_state(&input);
        if self.negotiated_version != derived.negotiated_version
            || self.verdict != derived.verdict
            || self.omissions != derived.omissions
            || self.uncertainty != derived.uncertainty
            || self.semantic_loss != derived.semantic_loss
            || self.reasons != derived.reasons
            || self.effect_receipt != derived.effect_receipt
        {
            return Err(DeterminismGatewayError::InvalidRequest(
                "gateway verdict is not derived from its retained input and policy".into(),
            ));
        }
        validate_sorted_strings("canonical_field_order", &self.canonical_field_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("reasons", &self.reasons)?;
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
            return Err(DeterminismGatewayError::InvalidRequest(
                "semantic-loss ordering is not canonical".into(),
            ));
        }
        let expected_version = match self.verdict {
            DeterminismGatewayVerdict::Accepted => CURRENT_INPUT_VERSION,
            DeterminismGatewayVerdict::Migrated => CURRENT_INPUT_VERSION,
            DeterminismGatewayVerdict::ApprovalRequired
            | DeterminismGatewayVerdict::Blocked
            | DeterminismGatewayVerdict::Incompatible => &self.negotiated_version,
        };
        if self.verdict == DeterminismGatewayVerdict::Accepted
            && self.negotiated_version != expected_version
        {
            return Err(DeterminismGatewayError::InvalidRequest(
                "accepted determinism version is not pinned to the current contract".into(),
            ));
        }
        if self.artifact.artifact_id != format!("typed-capability:{}", self.capability_id)
            || self.artifact.content_type
                != "application/vnd.aurora.canonical-capability-output+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance
                != vec![ProvenanceLink {
                    source_id: self.capability_id.clone(),
                    relation: "canonical-capability-input".into(),
                    digest: self.canonical_input_digest.clone(),
                }]
        {
            return Err(DeterminismGatewayError::Contract(
                "determinism artifact is not bound to the canonical output".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| DeterminismGatewayError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&canonical_output_payload(self))
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

fn validate_text(field: &str, value: &str) -> Result<(), DeterminismGatewayError> {
    if value.is_empty() || value.trim() != value {
        return Err(DeterminismGatewayError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(DeterminismGatewayError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), DeterminismGatewayError> {
    if values.len() > MAX_FIELDS {
        return Err(DeterminismGatewayError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    for value in values {
        validate_text(field, value)?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DeterminismGatewayError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedGatewayState {
    negotiated_version: String,
    verdict: DeterminismGatewayVerdict,
    omissions: Vec<String>,
    uncertainty: Vec<String>,
    semantic_loss: Vec<SemanticLoss>,
    reasons: Vec<String>,
    effect_receipt: String,
}

fn derive_gateway_state(input: &TypedCapabilityInput) -> DerivedGatewayState {
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
        input.values.len()
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
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    semantic_loss.sort_by(|left, right| {
        (left.field.as_str(), left.reason.as_str(), left.severity).cmp(&(
            right.field.as_str(),
            right.reason.as_str(),
            right.severity,
        ))
    });
    reasons.sort();
    reasons.dedup();
    let effect_receipt = if matches!(
        verdict,
        DeterminismGatewayVerdict::Accepted | DeterminismGatewayVerdict::Migrated
    ) {
        "exchange:permitted-artifacts"
    } else {
        "block:unauthorized-or-incompatible-exchange"
    };
    DerivedGatewayState {
        negotiated_version,
        verdict,
        omissions,
        uncertainty,
        semantic_loss,
        reasons,
        effect_receipt: effect_receipt.into(),
    }
}

fn canonical_output_payload(output: &CanonicalCapabilityOutput) -> serde_json::Value {
    canonical_output_payload_from_parts(
        &output.schema_version,
        &output.contract_version,
        &output.feature_id,
        &output.capability_id,
        &output.endpoint_id,
        &output.source_contract_version,
        &output.values,
        output.permitted_export,
        &output.negotiated_version,
        output.verdict,
        &output.canonical_field_order,
        &output.canonical_input_digest,
        &output.omissions,
        &output.uncertainty,
        &output.semantic_loss,
        &output.reasons,
        &output.effect_receipt,
        &output.artifact.provenance,
        output.raw_data_local,
        &output.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn canonical_output_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    capability_id: &str,
    endpoint_id: &str,
    source_contract_version: &str,
    values: &BTreeMap<String, serde_json::Value>,
    permitted_export: bool,
    negotiated_version: &str,
    verdict: DeterminismGatewayVerdict,
    canonical_field_order: &[String],
    canonical_input_digest: &ContentHash,
    omissions: &[String],
    uncertainty: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    effect_receipt: &str,
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "capability_id": capability_id,
        "endpoint_id": endpoint_id,
        "source_contract_version": source_contract_version,
        "values": values,
        "permitted_export": permitted_export,
        "negotiated_version": negotiated_version,
        "verdict": verdict,
        "canonical_field_order": canonical_field_order,
        "canonical_input_digest": canonical_input_digest,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "effect_receipt": effect_receipt,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    let derived = derive_gateway_state(input);
    let provenance = vec![ProvenanceLink {
        source_id: input.capability_id.clone(),
        relation: "canonical-capability-input".into(),
        digest: input_digest.clone(),
    }];
    let payload = canonical_output_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &input.capability_id,
        &input.endpoint_id,
        &input.source_contract_version,
        &input.values,
        input.permitted_export,
        &derived.negotiated_version,
        derived.verdict,
        &field_order,
        &input_digest,
        &derived.omissions,
        &derived.uncertainty,
        &derived.semantic_loss,
        &derived.reasons,
        &derived.effect_receipt,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("typed-capability:{}", input.capability_id),
        "application/vnd.aurora.canonical-capability-output+json",
        &payload,
        derived.semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| DeterminismGatewayError::Contract(error.to_string()))?;
    let output = CanonicalCapabilityOutput {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        capability_id: input.capability_id.clone(),
        endpoint_id: input.endpoint_id.clone(),
        source_contract_version: input.source_contract_version.clone(),
        values: input.values.clone(),
        permitted_export: input.permitted_export,
        negotiated_version: derived.negotiated_version,
        verdict: derived.verdict,
        canonical_field_order: field_order,
        canonical_input_digest: input_digest,
        omissions: derived.omissions,
        uncertainty: derived.uncertainty,
        semantic_loss: derived.semantic_loss,
        reasons: derived.reasons,
        effect_receipt: derived.effect_receipt,
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
    validate_text("capability_id", &input.capability_id)?;
    validate_text("source_contract_version", &input.source_contract_version)?;
    validate_text("endpoint_id", &input.endpoint_id)?;
    validate_text("boundary", &input.boundary)?;
    if input.values.len() > MAX_FIELDS {
        return Err(DeterminismGatewayError::InvalidRequest(
            "capability field count exceeds its bound".into(),
        ));
    }
    for key in input.values.keys() {
        validate_text("capability field", key)?;
    }
    let encoded = serde_json::to_vec(&input.values)
        .map_err(|error| DeterminismGatewayError::Serialization(error.to_string()))?;
    if encoded.len() > MAX_VALUE_BYTES {
        return Err(DeterminismGatewayError::InvalidRequest(
            "canonical capability values exceed their size bound".into(),
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

    #[test]
    fn output_rejects_tampered_artifact_payload_binding() {
        let mut output = negotiate_capability(&input()).unwrap();
        output.endpoint_id = "tampered-endpoint".into();
        let error = output.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn output_rejects_a_mismatched_effect_receipt() {
        let mut output = negotiate_capability(&input()).unwrap();
        output.effect_receipt = "block:unauthorized-or-incompatible-exchange".into();
        let error = output.validate().unwrap_err();
        assert!(error.to_string().contains("effect receipt"));
    }

    #[test]
    fn empty_capability_field_names_are_rejected() {
        let mut input = input();
        input.values.insert(" ".into(), json!(true));
        let error = negotiate_capability(&input).unwrap_err();
        assert!(error.to_string().contains("capability field"));
    }

    #[test]
    fn typed_values_are_bound_to_the_gateway_output() {
        let mut output = negotiate_capability(&input()).unwrap();
        output.values.insert("tampered".into(), json!(true));
        assert!(output.validate().is_err());
    }

    #[test]
    fn permission_state_is_bound_to_the_gateway_verdict() {
        let mut output = negotiate_capability(&input()).unwrap();
        output.permitted_export = false;
        assert!(output.validate().is_err());
    }

    #[test]
    fn gateway_input_provenance_tampering_is_rejected() {
        let mut output = negotiate_capability(&input()).unwrap();
        output.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(output.validate().is_err());
    }
}
