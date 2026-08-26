//! Multimodal multi-study knowledge-representation contract model.
//!
//! Atlas feature: `AFA-brain-P04-F06`. The contract boundary makes study×modality
//! comparability and semantic-profile migration explicit before a typed world is used.

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

pub const FEATURE_ID: &str = "AFA-brain-P04-F06";
pub const CONTRACT_VERSION: &str = "brain-multimodal-knowledge-representation-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ScopedResearchClaims1@1";
pub const OUTPUT_SCHEMA: &str = "TypedKnowledgeWorld1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalKnowledgeContractCell {
    pub cell_id: String,
    pub study_id: String,
    pub modality: String,
    pub claim_id: String,
    pub semantic_profile: String,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub state: EvidenceState,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalKnowledgeContractRequest {
    pub request_id: String,
    pub workspace_id: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub cells: Vec<MultimodalKnowledgeContractCell>,
    pub required_cell_ids: Vec<String>,
    pub input_schema: String,
    pub output_schema: String,
    pub source_revision: u16,
    pub target_revision: u16,
    pub migration_requested: bool,
    pub comparability_required: bool,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalKnowledgeContractDisposition {
    Compatible,
    Migrated,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalKnowledgeContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub disposition: MultimodalKnowledgeContractDisposition,
    pub input_schema: String,
    pub output_schema: String,
    pub source_revision: u16,
    pub target_revision: u16,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub contract_digest: ContentHash,
    pub comparability_digest: ContentHash,
    pub migration_digest: ContentHash,
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
pub enum MultimodalKnowledgeContractError {
    #[error("invalid multimodal knowledge contract: {0}")]
    Invalid(String),
    #[error("multimodal knowledge contract artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalKnowledgeContractReceipt {
    pub fn validate(&self) -> Result<(), MultimodalKnowledgeContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.source_revision == 0
            || self.target_revision < self.source_revision
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalKnowledgeContractError::Invalid("multimodal contract identity, study/modality coverage, schema, revision, locality, or effects are incomplete".into()));
        }
        for values in [
            &self.study_order, &self.modality_order, &self.candidate_order,
            &self.admitted_order, &self.unresolved_order, &self.denied_order,
            &self.missing_order, &self.incomparable_order, &self.semantic_loss_order,
            &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) { return Err(MultimodalKnowledgeContractError::Invalid("multimodal contract ordering is not canonical".into())); }
        }
        let classified = self.admitted_order.iter().chain(self.unresolved_order.iter()).chain(self.denied_order.iter()).cloned().collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len() || classified.iter().any(|cell| !self.candidate_order.contains(cell)) || self.missing_order.iter().any(|cell| !self.candidate_order.contains(cell)) || self.incomparable_order.iter().any(|cell| !self.candidate_order.contains(cell)) {
            return Err(MultimodalKnowledgeContractError::Invalid("multimodal contract states do not partition cells".into()));
        }
        for digest in [&self.contract_digest, &self.comparability_digest, &self.migration_digest, &self.replay_identity, &self.artifact.content_hash] { if digest.as_str().len() != 64 { return Err(MultimodalKnowledgeContractError::Invalid("multimodal contract digest is invalid".into())); } }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("read:local-multimodal-knowledge-contract:") && effect != "block:unsafe-release") { return Err(MultimodalKnowledgeContractError::Invalid("multimodal contract effect is outside local read gate".into())); }
        self.artifact.validate_metadata().map_err(|error| MultimodalKnowledgeContractError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalKnowledgeContractError> { self.validate()?; let value = serde_json::to_value(self).map_err(|error| MultimodalKnowledgeContractError::Artifact(error.to_string()))?; ContentHash::of_value(&value).map_err(|error| MultimodalKnowledgeContractError::Artifact(error.to_string())) }
}

pub fn multimodal_knowledge_representation_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["platform reliability engineer".into(), "multimodal workflow compiler".into()].into(), behavior: "validates versioned study×modality knowledge-world contracts with explicit comparability and migration loss".into(), value: "prevents incompatible imaging and omics semantics from being silently merged into one research world".into(), inputs: vec![TypedPort { name: "multimodal_scoped_research_claims".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "multimodal_typed_knowledge_world_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_multimodal_knowledge_representation_contract(request: &MultimodalKnowledgeContractRequest) -> Result<MultimodalKnowledgeContractReceipt, MultimodalKnowledgeContractError> {
    if request.request_id.trim().is_empty() || request.workspace_id.trim().is_empty() || request.study_order.len() < 2 || request.modality_order.len() < 2 || request.cells.is_empty() || request.input_schema != INPUT_SCHEMA || request.output_schema != OUTPUT_SCHEMA || request.source_revision == 0 || request.target_revision < request.source_revision || (request.target_revision > request.source_revision && !request.migration_requested) || request.replay_identity.as_str().len() != 64 || !request.raw_data_local || request.boundary != PRECLINICAL_BOUNDARY { return Err(MultimodalKnowledgeContractError::Invalid("multimodal contract identity, schemas, coverage, revisions, migration, replay, locality, or boundary is invalid".into())); }
    let mut studies = request.study_order.clone(); studies.sort(); let mut modalities = request.modality_order.clone(); modalities.sort(); if studies.windows(2).any(|p| p[0] == p[1]) || modalities.windows(2).any(|p| p[0] == p[1]) { return Err(MultimodalKnowledgeContractError::Invalid("study and modality identifiers must be unique".into())); }
    let mut cells = request.cells.clone(); cells.sort_by(|left, right| left.cell_id.cmp(&right.cell_id)); let candidate = cells.iter().map(|cell| cell.cell_id.clone()).collect::<Vec<_>>(); if candidate.windows(2).any(|p| p[0] == p[1]) || candidate.iter().any(|value| value.trim().is_empty()) { return Err(MultimodalKnowledgeContractError::Invalid("multimodal cell identifiers must be unique and non-empty".into())); }
    let profiles = cells.iter().map(|cell| cell.semantic_profile.clone()).collect::<BTreeSet<_>>(); let profile_conflict = request.comparability_required && profiles.len() > 1;
    let mut admitted = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut denied = BTreeSet::new(); let mut missing = BTreeSet::new(); let mut incomparable = BTreeSet::new(); let mut semantic_loss = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::from(["gate:study-modality-coverage".to_string(), "gate:schema-compatibility".to_string(), "gate:unknown-is-not-asserted".to_string(), "gate:locality".to_string()]); let mut negative = BTreeSet::new();
    for cell in &cells { if !request.study_order.contains(&cell.study_id) || !request.modality_order.contains(&cell.modality) || !request.policy_allow || !request.protected_closure || cell.boundary != PRECLINICAL_BOUNDARY { denied.insert(cell.cell_id.clone()); negative.insert(format!("cell:{}:scope-policy-closure", cell.cell_id)); } else if cell.evidence_digest.is_none() || cell.provenance_digest.is_none() { unresolved.insert(cell.cell_id.clone()); missing.insert(cell.cell_id.clone()); omissions.insert(format!("cell:{}:evidence-or-provenance-missing", cell.cell_id)); } else if matches!(cell.state, EvidenceState::Unknown | EvidenceState::Speculative) { unresolved.insert(cell.cell_id.clone()); uncertainty.insert(format!("cell:{}:unknown-not-asserted", cell.cell_id)); } else if matches!(cell.state, EvidenceState::Contradicted) { denied.insert(cell.cell_id.clone()); negative.insert(format!("cell:{}:contradicted", cell.cell_id)); } else if profile_conflict { unresolved.insert(cell.cell_id.clone()); incomparable.insert(cell.cell_id.clone()); uncertainty.insert(format!("cell:{}:semantic-profile-conflict", cell.cell_id)); } else { admitted.insert(cell.cell_id.clone()); if request.target_revision > request.source_revision { semantic_loss.insert(cell.cell_id.clone()); } } }
    for required in request.required_cell_ids.iter().collect::<BTreeSet<_>>() { if !candidate.contains(required) { omissions.insert(format!("cell:{}:required-missing", required)); uncertainty.insert(format!("cell:{}:required-unresolved", required)); } else if !admitted.contains(required) { uncertainty.insert(format!("cell:{}:required-not-admitted", required)); } }
    if request.target_revision > request.source_revision { uncertainty.insert(format!("migration:{}-to-{}", request.source_revision, request.target_revision)); } if profile_conflict { omissions.insert("control:semantic-profile-incompatibility".into()); } if !request.policy_allow { omissions.insert("control:policy-denied".into()); } if !request.protected_closure { omissions.insert("control:protected-closure-incomplete".into()); }
    let comparability_digest = ContentHash::of_value(&json!({"study_order": studies, "modality_order": modalities, "semantic_profiles": profiles, "required": request.comparability_required})).map_err(|error| MultimodalKnowledgeContractError::Artifact(error.to_string()))?; let contract_digest = ContentHash::of_value(&json!({"workspace_id": request.workspace_id, "candidate_order": candidate, "admitted_order": admitted, "unresolved_order": unresolved, "denied_order": denied, "incomparable_order": incomparable, "source_revision": request.source_revision, "target_revision": request.target_revision, "comparability_digest": comparability_digest})).map_err(|error| MultimodalKnowledgeContractError::Artifact(error.to_string()))?; let migration_digest = ContentHash::of_value(&json!({"source_revision": request.source_revision, "target_revision": request.target_revision, "migration_requested": request.migration_requested, "semantic_loss_order": semantic_loss})).map_err(|error| MultimodalKnowledgeContractError::Artifact(error.to_string()))?;
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { MultimodalKnowledgeContractDisposition::Blocked } else if admitted.is_empty() || !unresolved.is_empty() || !denied.is_empty() || profile_conflict { MultimodalKnowledgeContractDisposition::Partial } else if request.target_revision > request.source_revision { MultimodalKnowledgeContractDisposition::Migrated } else { MultimodalKnowledgeContractDisposition::Compatible };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workspace_id": request.workspace_id, "disposition": disposition, "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "source_revision": request.source_revision, "target_revision": request.target_revision, "comparability_digest": comparability_digest, "contract_digest": contract_digest, "migration_digest": migration_digest, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY}); let artifact = TypedResearchArtifact::from_payload(format!("brain-multimodal-knowledge-contract:{}", request.request_id), "application/vnd.aurora.typed-knowledge-world+json", &payload, Vec::new(), Vec::new()).map_err(|error| MultimodalKnowledgeContractError::Artifact(error.to_string()))?;
    let receipt = MultimodalKnowledgeContractReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), workspace_id: request.workspace_id.clone(), disposition, input_schema: INPUT_SCHEMA.into(), output_schema: OUTPUT_SCHEMA.into(), source_revision: request.source_revision, target_revision: request.target_revision, study_order: studies, modality_order: modalities, candidate_order: candidate, admitted_order: admitted.into_iter().collect(), unresolved_order: unresolved.into_iter().collect(), denied_order: denied.into_iter().collect(), missing_order: missing.into_iter().collect(), incomparable_order: incomparable.into_iter().collect(), semantic_loss_order: semantic_loss.into_iter().collect(), contract_digest, comparability_digest, migration_digest, replay_identity: request.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: if disposition == MultimodalKnowledgeContractDisposition::Blocked { vec!["block:unsafe-release".into()] } else { vec![format!("read:local-multimodal-knowledge-contract:{}", request.request_id)] }, artifact, raw_data_local: request.raw_data_local, boundary: PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

#[cfg(test)] mod tests { use super::*; fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) } fn request() -> MultimodalKnowledgeContractRequest { let h=hash("multimodal-contract"); let cell=|id:&str,study:&str,modality:&str,state:EvidenceState| MultimodalKnowledgeContractCell { cell_id:id.into(), study_id:study.into(), modality:modality.into(), claim_id:format!("claim:{id}"), semantic_profile:"profile:v1".into(), evidence_digest:Some(h.clone()), provenance_digest:Some(h.clone()), state, boundary:PRECLINICAL_BOUNDARY.into() }; MultimodalKnowledgeContractRequest { request_id:"request:multimodal-contract".into(), workspace_id:"workspace:one".into(), study_order:vec!["study:a".into(),"study:b".into()], modality_order:vec!["imaging".into(),"omics".into()], cells:vec![cell("cell:a","study:a","imaging",EvidenceState::Supported),cell("cell:b","study:b","omics",EvidenceState::Supported)], required_cell_ids:vec!["cell:a".into()], input_schema:INPUT_SCHEMA.into(), output_schema:OUTPUT_SCHEMA.into(), source_revision:1, target_revision:1, migration_requested:false, comparability_required:true, replay_identity:h, policy_allow:true, protected_closure:true, raw_data_local:true, boundary:PRECLINICAL_BOUNDARY.into() } } #[test] fn manifest_is_a1(){assert_eq!(multimodal_knowledge_representation_contract_model_manifest().autonomy_tier,AutonomyTier::A1);} #[test] fn comparable_contract_is_compatible(){assert_eq!(model_multimodal_knowledge_representation_contract(&request()).unwrap().disposition,MultimodalKnowledgeContractDisposition::Compatible);} #[test] fn migration_is_explicit(){let mut v=request();v.target_revision=2;v.migration_requested=true;assert_eq!(model_multimodal_knowledge_representation_contract(&v).unwrap().disposition,MultimodalKnowledgeContractDisposition::Migrated);} #[test] fn semantic_profile_conflict_is_partial(){let mut v=request();v.cells[1].semantic_profile="profile:v2".into();let r=model_multimodal_knowledge_representation_contract(&v).unwrap();assert_eq!(r.disposition,MultimodalKnowledgeContractDisposition::Partial);assert!(!r.incomparable_order.is_empty());} #[test] fn missing_modality_evidence_is_partial(){let mut v=request();v.cells[0].evidence_digest=None;let r=model_multimodal_knowledge_representation_contract(&v).unwrap();assert_eq!(r.disposition,MultimodalKnowledgeContractDisposition::Partial);assert_eq!(r.missing_order,vec!["cell:a".to_string()]);} #[test] fn unknown_is_unresolved(){let mut v=request();v.cells[0].state=EvidenceState::Unknown;let r=model_multimodal_knowledge_representation_contract(&v).unwrap();assert_eq!(r.disposition,MultimodalKnowledgeContractDisposition::Partial);} #[test] fn policy_blocks(){let mut v=request();v.policy_allow=false;let r=model_multimodal_knowledge_representation_contract(&v).unwrap();assert_eq!(r.disposition,MultimodalKnowledgeContractDisposition::Blocked);} #[test] fn digest_is_stable(){let r=model_multimodal_knowledge_representation_contract(&request()).unwrap();assert_eq!(r.digest().unwrap(),r.digest().unwrap());} }
