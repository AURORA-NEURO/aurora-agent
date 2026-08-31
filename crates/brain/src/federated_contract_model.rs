//! Federated continual evidence contract model.
//!
//! Atlas feature: `AFA-brain-P01-F08`. This typed primitive separates local contract validity
//! from export eligibility and never treats a signer or purpose failure as a warning-only state.

use crate::evidence_contract_model::ContractCompatibility;
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F08";
pub const CONTRACT_VERSION: &str = "brain-federated-evidence-contract/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";
pub const PERMITTED_ARTIFACT: &str = "qualified-evidence-summary";
const CONTRACT_CONTENT_TYPE: &str = "application/vnd.aurora.federated-evidence-contract+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContractModelRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: ContractCompatibility,
    pub required_fields: Vec<String>,
    pub provided_fields: Vec<String>,
    pub allowed_artifacts: Vec<String>,
    pub signer_valid: bool,
    pub semantic_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContractDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContractModelReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub disposition: FederatedContractDisposition,
    pub compatibility: ContractCompatibility,
    pub input_schema: String,
    pub output_schema: String,
    pub required_order: Vec<String>,
    pub provided_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub allowed_artifact_order: Vec<String>,
    pub export_scope: String,
    pub semantic_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub contract_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContractModelError {
    #[error("invalid federated evidence contract: {0}")]
    Invalid(String),
    #[error("federated contract artifact failed: {0}")]
    Artifact(String),
}

impl FederatedContractModelReceipt {
    pub fn validate(&self) -> Result<(), FederatedContractModelError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.required_order.is_empty()
            || self.provided_order.is_empty()
            || self.allowed_artifact_order.is_empty()
            || self.effect_receipts.len() != 1
        {
            return Err(FederatedContractModelError::Invalid("federation identity, schemas, fields, artifact policy, export scope, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.endpoint, "endpoint"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.export_scope, "export_scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for values in [
            &self.required_order,
            &self.provided_order,
            &self.missing_order,
            &self.semantic_loss_order,
            &self.allowed_artifact_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            validate_sorted_unique(values, "federated contract collection")?;
        }
        let required = self.required_order.iter().cloned().collect::<BTreeSet<_>>();
        let provided = self.provided_order.iter().cloned().collect::<BTreeSet<_>>();
        let expected_missing = required.difference(&provided).cloned().collect::<Vec<_>>();
        let expected_semantic_loss = provided.difference(&required).cloned().collect::<Vec<_>>();
        if self.missing_order != expected_missing
            || self.semantic_loss_order != expected_semantic_loss
        {
            return Err(FederatedContractModelError::Invalid(
                "federated contract loss state is not bound to declared fields".into(),
            ));
        }
        let gate_blocked = self.negative_evidence.iter().any(|item| {
            matches!(
                item.as_str(),
                "request:signer-invalid"
                    | "request:policy-denied"
                    | "request:raw-data-locality-failed"
            )
        }) || self
            .uncertainty
            .iter()
            .any(|item| item == "request:protected-closure-incomplete")
            || self
                .omissions
                .iter()
                .any(|item| item == "federation:permitted-artifact-missing");
        let expected_disposition = if gate_blocked {
            FederatedContractDisposition::Blocked
        } else if self.provided_order.is_empty() {
            FederatedContractDisposition::Unknown
        } else if self.missing_order.is_empty()
            && self.semantic_loss_order.is_empty()
            && self.compatibility == ContractCompatibility::Additive
        {
            FederatedContractDisposition::Qualified
        } else {
            FederatedContractDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(FederatedContractModelError::Invalid(
                "federated contract disposition does not match retained gate evidence".into(),
            ));
        }
        if self.export_scope
            != format!(
                "{}:{}:{}",
                self.federation_id, self.institution_id, self.purpose
            )
        {
            return Err(FederatedContractModelError::Invalid(
                "federated export scope is not bound to identity".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(FederatedContractModelError::Invalid(
                "federated contract receipts must declare local emitted data".into(),
            ));
        }
        let expected_effect = if self.disposition == FederatedContractDisposition::Qualified {
            format!("exchange:permitted-artifacts:{}", self.request_id)
        } else {
            "block:unsafe-release".into()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(FederatedContractModelError::Invalid(
                "federated contract effect does not match disposition".into(),
            ));
        }
        for digest in [
            &self.semantic_digest,
            &self.provenance_digest,
            &self.contract_digest,
            &self.envelope_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedContractModelError::Invalid(
                    "federated contract digest is invalid".into(),
                ));
            }
        }
        let expected_contract_digest = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "institution_id": self.institution_id,
            "purpose": self.purpose,
            "semantic_profile": self.semantic_profile,
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "compatibility": self.compatibility,
            "required_order": self.required_order,
            "provided_order": self.provided_order,
            "allowed_artifact_order": self.allowed_artifact_order,
        }))
        .map_err(|error| FederatedContractModelError::Artifact(error.to_string()))?;
        if self.contract_digest != expected_contract_digest {
            return Err(FederatedContractModelError::Invalid(
                "federated contract digest is not bound to contract state".into(),
            ));
        }
        let expected_envelope_digest = ContentHash::of_value(&json!({
            "export_scope": self.export_scope,
            "semantic_digest": self.semantic_digest,
            "provenance_digest": self.provenance_digest,
            "contract_digest": self.contract_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| FederatedContractModelError::Artifact(error.to_string()))?;
        if self.envelope_digest != expected_envelope_digest {
            return Err(FederatedContractModelError::Invalid(
                "federated envelope digest is not bound to contract state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-federated-contract:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTRACT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedContractModelError::Invalid(
                "federated contract artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedContractModelError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedContractModelError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedContractModelError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedContractModelError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedContractModelError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedContractModelError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedContractModelError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedContractModelError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedContractModelError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedContractModelError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedContractModelError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedContractModelReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "endpoint": receipt.endpoint,
        "semantic_profile": receipt.semantic_profile,
        "disposition": receipt.disposition,
        "compatibility": receipt.compatibility,
        "input_schema": receipt.input_schema,
        "output_schema": receipt.output_schema,
        "required_order": receipt.required_order,
        "provided_order": receipt.provided_order,
        "missing_order": receipt.missing_order,
        "semantic_loss_order": receipt.semantic_loss_order,
        "allowed_artifact_order": receipt.allowed_artifact_order,
        "export_scope": receipt.export_scope,
        "semantic_digest": receipt.semantic_digest,
        "provenance_digest": receipt.provenance_digest,
        "contract_digest": receipt.contract_digest,
        "envelope_digest": receipt.envelope_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn federated_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "federation steward".into()].into(), behavior: "models federated EvidenceFeed contracts with purpose, signer, artifact allow-list, semantic profile, locality, and export eligibility closure".into(), value: "prevents unauthorized or semantically ambiguous research artifacts from crossing institution boundaries".into(), inputs: vec![TypedPort { name: "federated_evidence_contract".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "federated_qualified_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["read:local-research-artifacts".into(), "export:permitted-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_federated_contract(
    request: &FederatedContractModelRequest,
) -> Result<FederatedContractModelReceipt, FederatedContractModelError> {
    validate_request(request)?;
    let required = request
        .required_fields
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let provided = request
        .provided_fields
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = required.difference(&provided).cloned().collect::<Vec<_>>();
    let semantic_loss = provided.difference(&required).cloned().collect::<Vec<_>>();
    let allowed_artifacts = request
        .allowed_artifacts
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let artifact_allowed = allowed_artifacts.contains(PERMITTED_ARTIFACT);
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|field| format!("field:{field}:required-missing")),
        );
    }
    if !semantic_loss.is_empty() {
        uncertainty.extend(
            semantic_loss
                .iter()
                .map(|field| format!("field:{field}:provided-not-declared")),
        );
    }
    if !artifact_allowed {
        omissions.insert("federation:permitted-artifact-missing".into());
    }
    if request.compatibility != ContractCompatibility::Additive {
        uncertainty.insert(format!(
            "contract:compatibility-{}",
            compatibility_label(request.compatibility)
        ));
    }
    if !request.signer_valid {
        negative.insert("request:signer-invalid".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let blocked_by_gate = !request.signer_valid
        || !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !artifact_allowed;
    let disposition = if blocked_by_gate {
        FederatedContractDisposition::Blocked
    } else if request.provided_fields.is_empty() {
        FederatedContractDisposition::Unknown
    } else if missing.is_empty()
        && semantic_loss.is_empty()
        && request.compatibility == ContractCompatibility::Additive
    {
        FederatedContractDisposition::Qualified
    } else {
        FederatedContractDisposition::Partial
    };
    let export_scope = format!(
        "{}:{}:{}",
        request.federation_id, request.institution_id, request.purpose
    );
    let contract_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": request.compatibility, "required_order": required, "provided_order": provided, "allowed_artifact_order": allowed_artifacts})).map_err(|error| FederatedContractModelError::Artifact(error.to_string()))?;
    let raw_data_local = true;
    let envelope_digest = ContentHash::of_value(&json!({"export_scope": export_scope, "semantic_digest": request.semantic_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| FederatedContractModelError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == FederatedContractDisposition::Qualified {
        vec![format!(
            "exchange:permitted-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "endpoint": request.endpoint, "semantic_profile": request.semantic_profile, "disposition": disposition, "compatibility": request.compatibility, "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "required_order": required, "provided_order": provided, "missing_order": missing, "semantic_loss_order": semantic_loss, "allowed_artifact_order": allowed_artifacts, "export_scope": export_scope, "semantic_digest": request.semantic_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "envelope_digest": envelope_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-contract:{}", request.request_id),
        CONTRACT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedContractModelError::Artifact(error.to_string()))?;
    let receipt = FederatedContractModelReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        purpose: request.purpose.clone(),
        endpoint: request.endpoint.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        compatibility: request.compatibility,
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        required_order: required.into_iter().collect(),
        provided_order: provided.into_iter().collect(),
        missing_order: missing,
        semantic_loss_order: semantic_loss,
        allowed_artifact_order: allowed_artifacts.into_iter().collect(),
        export_scope,
        semantic_digest: request.semantic_digest.clone(),
        provenance_digest: request.provenance_digest.clone(),
        contract_digest,
        envelope_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedContractModelRequest,
) -> Result<(), FederatedContractModelError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.required_fields.is_empty()
        || request.provided_fields.is_empty()
        || request.allowed_artifacts.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContractModelError::Invalid("federation identity, purpose, endpoint, schemas, fields, artifact policy, or boundary is incomplete".into()));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.federation_id, "federation_id"),
        (&request.institution_id, "institution_id"),
        (&request.purpose, "purpose"),
        (&request.endpoint, "endpoint"),
        (&request.semantic_profile, "semantic_profile"),
        (&request.input_schema, "input_schema"),
        (&request.output_schema, "output_schema"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_sorted_unique(&request.required_fields, "required_fields")?;
    validate_sorted_unique(&request.provided_fields, "provided_fields")?;
    validate_sorted_unique(&request.allowed_artifacts, "allowed_artifacts")?;
    for digest in [
        &request.semantic_digest,
        &request.provenance_digest,
        &request.replay_identity,
    ] {
        if digest.as_str().len() != 64 {
            return Err(FederatedContractModelError::Invalid(
                "federated contract digest is invalid".into(),
            ));
        }
    }
    Ok(())
}
fn compatibility_label(value: ContractCompatibility) -> &'static str {
    match value {
        ContractCompatibility::Additive => "additive",
        ContractCompatibility::MigrationRequired => "migration_required",
        ContractCompatibility::Breaking => "breaking",
        ContractCompatibility::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> FederatedContractModelRequest {
        FederatedContractModelRequest {
            request_id: "request:federated-contract".into(),
            federation_id: "federation:commons".into(),
            institution_id: "institution:a".into(),
            purpose: "benchmarking".into(),
            endpoint: "https://hub.example/research".into(),
            semantic_profile: "preclinical-evidence/v1".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            compatibility: ContractCompatibility::Additive,
            required_fields: vec![
                "contract_digest".into(),
                "provenance_digest".into(),
                "semantic_digest".into(),
            ],
            provided_fields: vec![
                "contract_digest".into(),
                "provenance_digest".into(),
                "semantic_digest".into(),
            ],
            allowed_artifacts: vec![PERMITTED_ARTIFACT.into()],
            signer_valid: true,
            semantic_digest: hash("semantic"),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1_and_exports_only_permitted_artifacts() {
        let manifest = federated_contract_model_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert!(manifest.effects.contains(&Effect::FederationExport));
    }
    #[test]
    fn signed_permitted_contract_qualifies() {
        let receipt = model_federated_contract(&request()).unwrap();
        assert_eq!(receipt.disposition, FederatedContractDisposition::Qualified);
        assert_eq!(receipt.effect_receipts.len(), 1);
    }
    #[test]
    fn missing_artifact_allow_list_blocks() {
        let mut input = request();
        input.allowed_artifacts = vec!["raw-data".into()];
        let receipt = model_federated_contract(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedContractDisposition::Blocked);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("permitted-artifact")));
    }
    #[test]
    fn signer_failure_blocks() {
        let mut input = request();
        input.signer_valid = false;
        let receipt = model_federated_contract(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedContractDisposition::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("signer")));
    }
    #[test]
    fn migration_is_partial_when_export_gates_close() {
        let mut input = request();
        input.compatibility = ContractCompatibility::MigrationRequired;
        let receipt = model_federated_contract(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedContractDisposition::Partial);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("migration_required")));
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request();
        input.raw_data_local = false;
        let receipt = model_federated_contract(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedContractDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item == "request:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn contract_and_payload_drift_are_rejected() {
        let receipt = model_federated_contract(&request()).unwrap();
        let mut digest_drift = receipt.clone();
        digest_drift.compatibility = ContractCompatibility::MigrationRequired;
        assert!(digest_drift.validate().is_err());

        let mut payload_drift = receipt;
        payload_drift.endpoint = "https://other.example/research".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn padded_contract_identity_is_rejected() {
        let mut input = request();
        input.request_id = " request:federated-contract".into();
        assert!(model_federated_contract(&input).is_err());
    }
    #[test]
    fn duplicate_artifact_policy_is_rejected() {
        let mut input = request();
        input.allowed_artifacts.push(PERMITTED_ARTIFACT.into());
        assert!(model_federated_contract(&input).is_err());
    }
}
