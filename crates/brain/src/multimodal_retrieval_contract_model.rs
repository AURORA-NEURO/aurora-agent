//! Typed multimodal retrieval contract model.
//!
//! Atlas feature: `AFA-brain-P02-F06`. Study/modality closure and comparability are validated
//! before multimodal synthesis, with semantic loss and incomplete coverage preserved.

use crate::evidence_contract_model::{ContractCompatibility, ContractDisposition};
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F06";
pub const CONTRACT_VERSION: &str = "brain-multimodal-retrieval-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery2@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalContractRequest {
    pub request_id: String,
    pub study_ids: Vec<String>,
    pub scope: String,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: ContractCompatibility,
    pub required_modalities: Vec<String>,
    pub provided_modalities: Vec<String>,
    pub semantic_digest: ContentHash,
    pub comparability_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_order: Vec<String>,
    pub scope: String,
    pub disposition: ContractDisposition,
    pub compatibility: ContractCompatibility,
    pub input_schema: String,
    pub output_schema: String,
    pub modality_required_order: Vec<String>,
    pub modality_provided_order: Vec<String>,
    pub modality_missing_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub semantic_digest: ContentHash,
    pub comparability_digest: ContentHash,
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
pub enum MultimodalRetrievalContractError {
    #[error("invalid multimodal retrieval contract: {0}")]
    Invalid(String),
    #[error("multimodal retrieval contract artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalRetrievalContractReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_order.len() < 2
            || self.scope.trim().is_empty()
            || self.modality_required_order.len() < 2
            || self.modality_provided_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalRetrievalContractError::Invalid("multimodal contract identity, schemas, study/modality closure, locality, or effects are incomplete".into()));
        }
        if self
            .modality_missing_order
            .iter()
            .any(|value| !self.modality_required_order.contains(value))
            || self
                .semantic_loss_order
                .iter()
                .any(|value| !self.modality_provided_order.contains(value))
        {
            return Err(MultimodalRetrievalContractError::Invalid(
                "multimodal contract loss state is outside declared modalities".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_required_order,
            &self.modality_provided_order,
            &self.modality_missing_order,
            &self.semantic_loss_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalRetrievalContractError::Invalid(
                    "multimodal contract ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-multimodal-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalRetrievalContractError::Invalid(
                "effect is outside multimodal contract gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalRetrievalContractError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalRetrievalContractError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalRetrievalContractError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalRetrievalContractError::Artifact(error.to_string()))
    }
}

pub fn multimodal_retrieval_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "multimodal retrieval operator".into()].into(), behavior: "validates multimodal retrieval schema, study/modality closure, semantic loss, comparability, provenance, and replay identity before synthesis".into(), value: "prevents incomplete or semantically incomparable imaging and omics retrieval data from entering a qualified workflow".into(), inputs: vec![TypedPort { name: "multimodal_retrieval_contract".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "multimodal_synthesis_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-multimodal-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_multimodal_retrieval_contract(
    request: &MultimodalRetrievalContractRequest,
) -> Result<MultimodalRetrievalContractReceipt, MultimodalRetrievalContractError> {
    validate_request(request)?;
    let studies = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let required = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let provided = request
        .provided_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = required.difference(&provided).cloned().collect::<Vec<_>>();
    let semantic_loss = provided.difference(&required).cloned().collect::<Vec<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|value| format!("modality:{value}:required-missing")),
        );
    }
    if !semantic_loss.is_empty() {
        uncertainty.extend(
            semantic_loss
                .iter()
                .map(|value| format!("modality:{value}:provided-not-declared")),
        );
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
    let disposition = if !request.policy_allow || !request.protected_closure {
        ContractDisposition::Blocked
    } else if !missing.is_empty()
        || matches!(request.compatibility, ContractCompatibility::Breaking)
    {
        ContractDisposition::Blocked
    } else if !semantic_loss.is_empty()
        || !matches!(request.compatibility, ContractCompatibility::Additive)
        || !negative.is_empty()
    {
        ContractDisposition::Partial
    } else {
        ContractDisposition::Qualified
    };
    let contract_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": studies, "required": required, "provided": provided, "compatibility": request.compatibility, "semantic_digest": request.semantic_digest, "comparability_digest": request.comparability_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "replay_identity": request.replay_identity, "disposition": disposition})).map_err(|error| MultimodalRetrievalContractError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": studies, "scope": request.scope, "disposition": disposition, "compatibility": request.compatibility, "input_schema": request.input_schema, "output_schema": request.output_schema, "modality_required_order": required, "modality_provided_order": provided, "modality_missing_order": missing, "semantic_loss_order": semantic_loss, "semantic_digest": request.semantic_digest, "comparability_digest": request.comparability_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-retrieval-contract:{}", request.request_id),
        "application/vnd.aurora.multimodal-retrieval-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalRetrievalContractError::Artifact(error.to_string()))?;
    let receipt = MultimodalRetrievalContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_order: studies.into_iter().collect(),
        scope: request.scope.clone(),
        disposition,
        compatibility: request.compatibility,
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        modality_required_order: required.into_iter().collect(),
        modality_provided_order: provided.into_iter().collect(),
        modality_missing_order: missing,
        semantic_loss_order: semantic_loss,
        semantic_digest: request.semantic_digest.clone(),
        comparability_digest: request.comparability_digest.clone(),
        artifact_digest: request.artifact_digest.clone(),
        provenance_digest: request.provenance_digest.clone(),
        contract_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(
            disposition,
            ContractDisposition::Qualified | ContractDisposition::Partial
        ) {
            vec![format!(
                "read:local-multimodal-artifacts:{}",
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
    request: &MultimodalRetrievalContractRequest,
) -> Result<(), MultimodalRetrievalContractError> {
    if request.request_id.trim().is_empty()
        || request.study_ids.len() < 2
        || request.scope.trim().is_empty()
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.required_modalities.len() < 2
        || request.provided_modalities.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalRetrievalContractError::Invalid(
            "multimodal contract identity, schemas, coverage, or boundary is incomplete".into(),
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
    fn request(provided_modalities: Vec<String>) -> MultimodalRetrievalContractRequest {
        MultimodalRetrievalContractRequest {
            request_id: "request:multimodal-retrieval-contract".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            scope: "organoid:neural".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            compatibility: ContractCompatibility::Additive,
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            provided_modalities,
            semantic_digest: hash("semantic"),
            comparability_digest: hash("comparability"),
            artifact_digest: hash("artifact"),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = multimodal_retrieval_contract_model_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_is_qualified() {
        let r = model_multimodal_retrieval_contract(&request(vec![
            "imaging".into(),
            "transcriptomics".into(),
        ]))
        .unwrap();
        assert_eq!(r.disposition, ContractDisposition::Qualified);
    }
    #[test]
    fn missing_modality_blocks() {
        let r = model_multimodal_retrieval_contract(&request(vec!["imaging".into()])).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
    }
    #[test]
    fn semantic_loss_is_partial() {
        let r = model_multimodal_retrieval_contract(&request(vec![
            "imaging".into(),
            "transcriptomics".into(),
            "electrophysiology".into(),
        ]))
        .unwrap();
        assert_eq!(r.disposition, ContractDisposition::Partial);
    }
    #[test]
    fn policy_blocks() {
        let mut q = request(vec!["imaging".into(), "transcriptomics".into()]);
        q.policy_allow = false;
        let r = model_multimodal_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = model_multimodal_retrieval_contract(&request(vec![
            "imaging".into(),
            "transcriptomics".into(),
        ]))
        .unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
