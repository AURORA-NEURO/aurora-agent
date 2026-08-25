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
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.permitted_artifact != PERMITTED_ARTIFACT
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedRetrievalContractError::Invalid("federation identity, schemas, purpose, coverage, permitted artifact, locality, or effects are incomplete".into()));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedRetrievalContractError::Invalid(
                    "federated contract ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedRetrievalContractError::Invalid(
                "effect is outside federated contract gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
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
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.signer_valid
        || !request.approval_valid
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
    let contract_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "study_order": studies, "modality_order": modalities, "comparability_digest": request.comparability_digest, "envelope_digest": request.envelope_digest, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "replay_identity": request.replay_identity, "disposition": disposition})).map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "endpoint": request.endpoint, "study_order": studies, "modality_order": modalities, "disposition": disposition, "compatibility": request.compatibility, "input_schema": request.input_schema, "output_schema": request.output_schema, "permitted_artifact": PERMITTED_ARTIFACT, "comparability_digest": request.comparability_digest, "envelope_digest": request.envelope_digest, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-retrieval-contract:{}", request.request_id),
        "application/vnd.aurora.federated-retrieval-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalContractError::Artifact(error.to_string()))?;
    let exchange_allowed = matches!(
        disposition,
        ContractDisposition::Qualified | ContractDisposition::Partial
    ) && !request
        .allowed_artifacts
        .iter()
        .any(|value| value != PERMITTED_ARTIFACT);
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
        effect_receipts: if exchange_allowed {
            vec![format!(
                "exchange:permitted-artifacts:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
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
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.study_ids.len() < 2
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
}
