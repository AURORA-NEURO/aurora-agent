//! Typed federated retrieval contract model.
//!
//! Atlas feature: `AFA-brain-P02-F08`. Purpose-bound aggregate-only exchange is validated before
//! a federated retrieval result can leave the institution-local boundary.

use crate::evidence_contract_model::{ContractCompatibility, ContractDisposition};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P02-F08";
pub const CONTRACT_VERSION: &str = "brain-federated-retrieval-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "FederatedRetrievalQuery1@1";
pub const OUTPUT_SCHEMA: &str = "FederatedEvidenceSynthesis1@1";
pub const PERMITTED_ARTIFACT: &str = "qualified-evidence-summary";
const CONTRACT_CONTENT_TYPE: &str = "application/vnd.aurora.federated-retrieval-contract+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalContractRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub allowed_artifacts: Vec<String>,
    pub study_ids: Vec<String>,
    pub required_modalities: Vec<String>,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: ContractCompatibility,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signer_valid: bool,
    pub approval_valid: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub allowed_artifact_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: ContractDisposition,
    pub compatibility: ContractCompatibility,
    pub input_schema: String,
    pub output_schema: String,
    pub permitted_artifact: String,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub contract_digest: ContentHash,
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
pub enum FederatedRetrievalContractError {
    #[error("invalid federated retrieval contract: {0}")]
    Invalid(String),
    #[error("federated retrieval contract artifact failed: {0}")]
    Artifact(String),
}

impl FederatedRetrievalContractReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.permitted_artifact != PERMITTED_ARTIFACT
        {
            return Err(FederatedRetrievalContractError::Invalid("federation identity, schemas, purpose, coverage, permitted artifact, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.endpoint, "endpoint"),
            (&self.input_schema, "input_schema"),
            (&self.output_schema, "output_schema"),
            (&self.permitted_artifact, "permitted_artifact"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        validate_sorted_unique(&self.study_order, "study_order")?;
        validate_sorted_unique(&self.modality_order, "modality_order")?;
        validate_sorted_unique(&self.allowed_artifact_order, "allowed_artifact_order")?;
        if self.disposition != ContractDisposition::Blocked
            && !self
                .allowed_artifact_order
                .iter()
                .any(|artifact| artifact == PERMITTED_ARTIFACT)
        {
            return Err(FederatedRetrievalContractError::Invalid(
                "permitted artifact is not present in the allowed artifact declaration".into(),
            ));
        }
        for (values, field) in [
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        if !self.raw_data_local && self.disposition != ContractDisposition::Blocked {
            return Err(FederatedRetrievalContractError::Invalid(
                "non-local federated contracts must be blocked".into(),
            ));
        }
        if !self.raw_data_local
            && !self
                .omissions
                .iter()
                .any(|omission| omission == "request:raw-data-locality-failed")
        {
            return Err(FederatedRetrievalContractError::Invalid(
                "non-local federated contracts must retain a locality omission".into(),
            ));
        }
        let exchange_allowed = matches!(
            self.disposition,
            ContractDisposition::Qualified | ContractDisposition::Partial
        ) && self.allowed_artifact_order
            == vec![PERMITTED_ARTIFACT.to_string()];
        let expected_effect_receipts = if exchange_allowed {
            vec![format!("exchange:permitted-artifacts:{}", self.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedRetrievalContractError::Invalid(
                "federated contract effect does not match artifact declaration and disposition"
                    .into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.semantic_digest,
            &self.artifact_digest,
            &self.provenance_digest,
            &self.contract_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalContractError::Invalid(
                    "federated contract digest is invalid".into(),
                ));
            }
        }
        let expected_contract_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "institution_id": self.institution_id,
            "purpose": self.purpose,
            "semantic_profile": self.semantic_profile,
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "allowed_artifact_order": self.allowed_artifact_order,
            "comparability_digest": self.comparability_digest,
            "envelope_digest": self.envelope_digest,
            "semantic_digest": self.semantic_digest,
            "artifact_digest": self.artifact_digest,
            "provenance_digest": self.provenance_digest,
            "replay_identity": self.replay_identity,
            "disposition": self.disposition,
        }))
        .map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))?;
        if self.contract_digest != expected_contract_digest {
            return Err(FederatedRetrievalContractError::Invalid(
                "federated contract digest is not bound to its declaration".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-federated-retrieval-contract:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTRACT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedRetrievalContractError::Invalid(
                "federated contract artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalContractError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedRetrievalContractError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedRetrievalContractError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedRetrievalContractError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedRetrievalContractError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedRetrievalContractError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedRetrievalContractError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedRetrievalContractReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "semantic_profile": receipt.semantic_profile,
        "endpoint": receipt.endpoint,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "disposition": receipt.disposition,
        "compatibility": receipt.compatibility,
        "input_schema": receipt.input_schema,
        "output_schema": receipt.output_schema,
        "permitted_artifact": receipt.permitted_artifact,
        "allowed_artifact_order": receipt.allowed_artifact_order,
        "comparability_digest": receipt.comparability_digest,
        "envelope_digest": receipt.envelope_digest,
        "semantic_digest": receipt.semantic_digest,
        "artifact_digest": receipt.artifact_digest,
        "provenance_digest": receipt.provenance_digest,
        "contract_digest": receipt.contract_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn federated_retrieval_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["federation steward".into(), "multisite retrieval operator".into()].into(), behavior: "validates federated retrieval schema, purpose, institution, signer, approval, permitted artifact, comparability, envelope, provenance, and replay identity before aggregate-only exchange".into(), value: "prevents raw-data egress and unapproved or incomparable federated evidence release".into(), inputs: vec![TypedPort { name: "federated_retrieval_contract".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "federated_synthesis_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["read:local-research-artifacts".into(), "export:permitted-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federated retrieval approver".into(), reason: "approve purpose-bound aggregate-only exchange after signer, comparability, and locality gates close".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_federated_retrieval_contract(
    request: &FederatedRetrievalContractRequest,
) -> Result<FederatedRetrievalContractReceipt, FederatedRetrievalContractError> {
    validate_request(request)?;
    let studies = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let allowed_artifacts = request
        .allowed_artifacts
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if !request
        .allowed_artifacts
        .iter()
        .any(|value| value == PERMITTED_ARTIFACT)
    {
        omissions.insert("request:permitted-artifact-missing".into());
    }
    if !matches!(request.compatibility, ContractCompatibility::Additive) {
        uncertainty.insert(format!(
            "contract:compatibility-{}",
            compatibility_label(request.compatibility)
        ));
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !request.signer_valid {
        omissions.insert("request:signer-invalid".into());
    }
    if !request.approval_valid {
        omissions.insert("request:approval-required".into());
    }
    if !request.raw_data_local {
        omissions.insert("request:raw-data-locality-failed".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.signer_valid
        || !request.approval_valid
        || !request.raw_data_local
        || !request
            .allowed_artifacts
            .iter()
            .any(|value| value == PERMITTED_ARTIFACT)
    {
        ContractDisposition::Blocked
    } else if !matches!(request.compatibility, ContractCompatibility::Additive)
        || !uncertainty.is_empty()
        || !negative.is_empty()
    {
        ContractDisposition::Partial
    } else {
        ContractDisposition::Qualified
    };
    let contract_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "study_order": studies, "modality_order": modalities, "allowed_artifact_order": allowed_artifacts, "comparability_digest": request.comparability_digest, "envelope_digest": request.envelope_digest, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "replay_identity": request.replay_identity, "disposition": disposition})).map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))?;
    let exchange_allowed = matches!(
        disposition,
        ContractDisposition::Qualified | ContractDisposition::Partial
    ) && allowed_artifacts.len() == 1
        && allowed_artifacts.contains(PERMITTED_ARTIFACT);
    let effect_receipts = if exchange_allowed {
        vec![format!(
            "exchange:permitted-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "endpoint": request.endpoint, "study_order": studies, "modality_order": modalities, "disposition": disposition, "compatibility": request.compatibility, "input_schema": request.input_schema, "output_schema": request.output_schema, "permitted_artifact": PERMITTED_ARTIFACT, "allowed_artifact_order": allowed_artifacts, "comparability_digest": request.comparability_digest, "envelope_digest": request.envelope_digest, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-retrieval-contract:{}", request.request_id),
        CONTRACT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))?;
    let receipt = FederatedRetrievalContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        endpoint: request.endpoint.clone(),
        allowed_artifact_order: allowed_artifacts.into_iter().collect(),
        study_order: studies.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        disposition,
        compatibility: request.compatibility,
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        permitted_artifact: PERMITTED_ARTIFACT.into(),
        comparability_digest: request.comparability_digest.clone(),
        envelope_digest: request.envelope_digest.clone(),
        semantic_digest: request.semantic_digest.clone(),
        artifact_digest: request.artifact_digest.clone(),
        provenance_digest: request.provenance_digest.clone(),
        contract_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedRetrievalContractRequest,
) -> Result<(), FederatedRetrievalContractError> {
    if request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedRetrievalContractError::Invalid(
            "federated contract identity, purpose, coverage, schemas, or boundary is incomplete"
                .into(),
        ));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.federation_id, "federation_id"),
        (&request.institution_id, "institution_id"),
        (&request.purpose, "purpose"),
        (&request.semantic_profile, "semantic_profile"),
        (&request.endpoint, "endpoint"),
        (&request.input_schema, "input_schema"),
        (&request.output_schema, "output_schema"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.study_ids, "study_ids")?;
    validate_unique(&request.required_modalities, "required_modalities")?;
    validate_unique(&request.allowed_artifacts, "allowed_artifacts")?;
    for digest in [
        &request.comparability_digest,
        &request.envelope_digest,
        &request.semantic_digest,
        &request.artifact_digest,
        &request.provenance_digest,
        &request.replay_identity,
    ] {
        if digest.as_str().len() != 64 {
            return Err(FederatedRetrievalContractError::Invalid(
                "federated contract request digest is invalid".into(),
            ));
        }
    }
    Ok(())
}
fn compatibility_label(value: ContractCompatibility) -> &'static str {
    match value {
        ContractCompatibility::Additive => "additive",
        ContractCompatibility::MigrationRequired => "migration-required",
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
    fn request() -> FederatedRetrievalContractRequest {
        FederatedRetrievalContractRequest {
            request_id: "request:federated-contract".into(),
            federation_id: "federation:consortium".into(),
            institution_id: "institution:local".into(),
            purpose: "preclinical replication benchmark".into(),
            semantic_profile: "ome-ngff:5".into(),
            endpoint: "https://federation.invalid/admit".into(),
            allowed_artifacts: vec![PERMITTED_ARTIFACT.into()],
            study_ids: vec!["study:a".into(), "study:b".into()],
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            compatibility: ContractCompatibility::Additive,
            comparability_digest: hash("comparability"),
            envelope_digest: hash("envelope"),
            semantic_digest: hash("semantic"),
            artifact_digest: hash("artifact"),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            approval_valid: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        let m = federated_retrieval_contract_model_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn complete_is_qualified() {
        let r = model_federated_retrieval_contract(&request()).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Qualified);
        assert!(r.effect_receipts[0].starts_with("exchange:"));
    }
    #[test]
    fn signer_blocks() {
        let mut q = request();
        q.signer_valid = false;
        let r = model_federated_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
    }
    #[test]
    fn approval_blocks() {
        let mut q = request();
        q.approval_valid = false;
        let r = model_federated_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
    }
    #[test]
    fn missing_artifact_blocks() {
        let mut q = request();
        q.allowed_artifacts.clear();
        let r = model_federated_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = model_federated_retrieval_contract(&request()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut q = request();
        q.raw_data_local = false;
        let r = model_federated_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
        assert!(r.raw_data_local);
        assert!(r
            .omissions
            .iter()
            .any(|value| value == "request:raw-data-locality-failed"));
        r.validate().unwrap();
    }
    #[test]
    fn contract_declaration_and_artifact_payload_are_bound() {
        let mut declaration_drift = model_federated_retrieval_contract(&request()).unwrap();
        declaration_drift
            .allowed_artifact_order
            .push("other-artifact".into());
        assert!(declaration_drift.validate().is_err());

        let mut payload_drift = model_federated_retrieval_contract(&request()).unwrap();
        payload_drift.endpoint = "https://federation.invalid/other".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn identity_aliases_are_rejected() {
        let mut q = request();
        q.study_ids.push("STUDY:A".into());
        assert!(model_federated_retrieval_contract(&q).is_err());
    }
}
