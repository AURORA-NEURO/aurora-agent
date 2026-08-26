//! Local single-study evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-adapter-P01-F05`. This is a typed schema/compatibility product surface,
//! distinct from the F01 inference engine: it proves that migrations retain uncertainty and
//! semantic loss instead of changing an evidence claim into a silent default.

use bioprism_foundation::{AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F05";
pub const CONTRACT_VERSION: &str = "adapter-local-evidence-surveillance-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractModelClaim {
    pub claim_id: String,
    pub semantic_type: String,
    pub value_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub omitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceContractRequest {
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub claims: Vec<ContractModelClaim>,
    pub required_claim_ids: Vec<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractCompatibilityDisposition { Compatible, AdditiveMigration, Breaking, Incompatible }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractModelDisposition { Compatible, Partial, Unknown, Blocked }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceContractReceipt {
    pub schema_version: String, pub contract_version: String, pub feature_id: String, pub request_id: String,
    pub input_schema: String, pub output_schema: String, pub compatibility: ContractCompatibilityDisposition,
    pub disposition: ContractModelDisposition, pub candidate_order: Vec<String>, pub retained_order: Vec<String>,
    pub unknown_order: Vec<String>, pub denied_order: Vec<String>, pub migration_order: Vec<String>, pub semantic_loss: Vec<String>,
    pub required_order: Vec<String>, pub contract_digest: ContentHash, pub canonical_digest: ContentHash,
    pub provenance_digest: ContentHash, pub replay_identity: ContentHash, pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact, pub raw_data_local: bool, pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocalEvidenceSurveillanceContractError { #[error("invalid local evidence contract request: {0}")] Invalid(String), #[error("local evidence contract artifact failed: {0}")] Artifact(String) }
fn sorted_unique(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }

impl LocalEvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), LocalEvidenceSurveillanceContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local || self.request_id.trim().is_empty() || self.candidate_order.is_empty() || self.effect_receipts.is_empty() || self.input_schema != INPUT_SCHEMA || self.output_schema != OUTPUT_SCHEMA { return Err(LocalEvidenceSurveillanceContractError::Invalid("contract identity, schemas, locality, candidates, or effects are incomplete".into())); }
        for values in [&self.candidate_order, &self.retained_order, &self.unknown_order, &self.denied_order, &self.migration_order, &self.semantic_loss, &self.required_order, &self.effect_receipts] { if !sorted_unique(values) { return Err(LocalEvidenceSurveillanceContractError::Invalid("contract ordering is not canonical".into())); } }
        let classified = self.retained_order.iter().chain(self.unknown_order.iter()).chain(self.denied_order.iter()).cloned().collect::<BTreeSet<_>>(); if classified != self.candidate_order.iter().cloned().collect() { return Err(LocalEvidenceSurveillanceContractError::Invalid("contract states do not partition candidates".into())); }
        for digest in [&self.contract_digest, &self.canonical_digest, &self.provenance_digest, &self.replay_identity, &self.artifact.content_hash] { if digest.as_str().len() != 64 { return Err(LocalEvidenceSurveillanceContractError::Invalid("contract digest is invalid".into())); } }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("read:local-contract:") && effect != "block:unsafe-release") { return Err(LocalEvidenceSurveillanceContractError::Invalid("contract effect is outside local-read gate".into())); }
        if self.disposition == ContractModelDisposition::Blocked && self.effect_receipts != vec!["block:unsafe-release".to_string()] { return Err(LocalEvidenceSurveillanceContractError::Invalid("blocked contract must be explicitly blocked".into())); }
        self.artifact.validate_metadata().map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, LocalEvidenceSurveillanceContractError> { self.validate()?; let value = serde_json::to_value(self).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?; ContentHash::of_value(&value).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string())) }
}

pub fn local_evidence_surveillance_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["integration engineer".into(), "schema steward".into()].into(), behavior: "validates EvidenceFeed1 into a versioned QualifiedEvidenceSet2 with deterministic compatibility and semantic-loss witnesses".into(), value: "makes additive migration, breaking schema changes, omissions, and unknown evidence machine-checkable".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "JSON Schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_local_evidence_surveillance_contract(request: &LocalEvidenceSurveillanceContractRequest) -> Result<LocalEvidenceSurveillanceContractReceipt, LocalEvidenceSurveillanceContractError> {
    if request.request_id.trim().is_empty() || request.input_schema.trim().is_empty() || request.output_schema.trim().is_empty() || request.claims.is_empty() || request.replay_identity.as_str().len() != 64 || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local { return Err(LocalEvidenceSurveillanceContractError::Invalid("contract identity, schemas, claims, replay, locality, or boundary is invalid".into())); }
    let mut claims = request.claims.clone(); claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id)); let claim_ids = claims.iter().map(|claim| claim.claim_id.clone()).collect::<Vec<_>>(); if claim_ids.iter().any(|value| value.trim().is_empty()) || claim_ids.windows(2).any(|pair| pair[0] == pair[1]) { return Err(LocalEvidenceSurveillanceContractError::Invalid("claim identities must be unique and non-empty".into())); } let mut candidate_set = claim_ids.into_iter().collect::<BTreeSet<_>>(); candidate_set.extend(request.required_claim_ids.iter().cloned()); let candidate_order = candidate_set.into_iter().collect::<Vec<_>>();
    let compatibility = if request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA { ContractCompatibilityDisposition::AdditiveMigration } else if request.input_schema == request.output_schema { ContractCompatibilityDisposition::Compatible } else { ContractCompatibilityDisposition::Breaking };
    let mut retained = BTreeSet::new(); let mut unknown = BTreeSet::new(); let mut denied = BTreeSet::new(); let mut migration = BTreeSet::new(); let mut semantic_loss = BTreeSet::new(); let required = request.required_claim_ids.iter().cloned().collect::<BTreeSet<_>>();
    for claim in &claims { if compatibility == ContractCompatibilityDisposition::Breaking { denied.insert(claim.claim_id.clone()); semantic_loss.insert(format!("claim:{}:breaking-schema", claim.claim_id)); } else if claim.omitted { unknown.insert(claim.claim_id.clone()); semantic_loss.insert(format!("claim:{}:omitted-preserved", claim.claim_id)); } else if matches!(claim.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative) { unknown.insert(claim.claim_id.clone()); semantic_loss.insert(format!("claim:{}:unknown-not-asserted", claim.claim_id)); } else if claim.evidence_state == EvidenceState::Contradicted { denied.insert(claim.claim_id.clone()); semantic_loss.insert(format!("claim:{}:contradicted-retained", claim.claim_id)); } else { retained.insert(claim.claim_id.clone()); if compatibility == ContractCompatibilityDisposition::AdditiveMigration { migration.insert(format!("claim:{}:evidence-state-preserved", claim.claim_id)); } if claim.negative_result { semantic_loss.insert(format!("claim:{}:negative-result-retained", claim.claim_id)); } } }
    let known = candidate_order.iter().cloned().collect::<BTreeSet<_>>(); for claim_id in &required { if !known.contains(claim_id) || !retained.contains(claim_id) { unknown.insert(claim_id.clone()); semantic_loss.insert(format!("claim:{}:required-unresolved", claim_id)); } }
    if !request.policy_allow { semantic_loss.insert("control:policy-denied".into()); } if !request.protected_closure { semantic_loss.insert("control:protected-closure-incomplete".into()); }
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { ContractModelDisposition::Blocked } else if retained.is_empty() { ContractModelDisposition::Unknown } else if compatibility == ContractCompatibilityDisposition::Breaking || !unknown.is_empty() || !denied.is_empty() { ContractModelDisposition::Partial } else { ContractModelDisposition::Compatible };
    let retained_order = retained.iter().cloned().collect::<Vec<_>>(); let unknown_order = unknown.iter().cloned().collect::<Vec<_>>(); let denied_order = denied.iter().cloned().collect::<Vec<_>>(); let migration_order = migration.iter().cloned().collect::<Vec<_>>(); let semantic_loss = semantic_loss.iter().cloned().collect::<Vec<_>>(); let required_order = required.iter().cloned().collect::<Vec<_>>(); let contract_digest = ContentHash::of_value(&json!({"input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": compatibility, "candidate_order": candidate_order.clone(), "required_order": required_order.clone()})).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?; let canonical_digest = ContentHash::of_value(&json!({"retained_order": retained_order.clone(), "unknown_order": unknown_order.clone(), "denied_order": denied_order.clone(), "migration_order": migration_order.clone(), "semantic_loss": semantic_loss.clone()})).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?; let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "contract_digest": contract_digest, "canonical_digest": canonical_digest})).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?; let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": compatibility, "disposition": disposition, "candidate_order": candidate_order, "retained_order": retained_order, "unknown_order": unknown_order, "denied_order": denied_order, "migration_order": migration_order, "semantic_loss": semantic_loss, "required_order": required_order, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": request.replay_identity, "raw_data_local": request.raw_data_local, "boundary": PRECLINICAL_BOUNDARY}); let artifact = TypedResearchArtifact::from_payload(format!("adapter-local-evidence-contract:{}", request.request_id), "application/vnd.aurora.qualified-evidence-set+json", &payload, Vec::new(), Vec::new()).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?; let receipt = LocalEvidenceSurveillanceContractReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), input_schema: INPUT_SCHEMA.into(), output_schema: OUTPUT_SCHEMA.into(), compatibility, disposition, candidate_order, retained_order, unknown_order, denied_order, migration_order, semantic_loss, required_order, contract_digest, canonical_digest, provenance_digest, replay_identity: request.replay_identity.clone(), effect_receipts: if disposition == ContractModelDisposition::Blocked { vec!["block:unsafe-release".into()] } else { vec![format!("read:local-contract:{}", request.request_id)] }, artifact, raw_data_local: request.raw_data_local, boundary: PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests { use super::*; fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) } fn request() -> LocalEvidenceSurveillanceContractRequest { let digest = hash("contract"); let claim = |id: &str, state: EvidenceState| ContractModelClaim { claim_id: id.into(), semantic_type: "evidence".into(), value_digest: digest.clone(), evidence_state: state, omitted: false, negative_result: false }; LocalEvidenceSurveillanceContractRequest { request_id: "request:contract".into(), input_schema: INPUT_SCHEMA.into(), output_schema: OUTPUT_SCHEMA.into(), claims: vec![claim("claim:a", EvidenceState::Supported), claim("claim:b", EvidenceState::Supported)], required_claim_ids: vec!["claim:a".into()], policy_allow: true, protected_closure: true, raw_data_local: true, replay_identity: digest, boundary: PRECLINICAL_BOUNDARY.into() } } #[test] fn manifest_is_a0() { assert_eq!(local_evidence_surveillance_contract_model_manifest().autonomy_tier, AutonomyTier::A0); } #[test] fn additive_model_is_compatible() { assert_eq!(model_local_evidence_surveillance_contract(&request()).unwrap().compatibility, ContractCompatibilityDisposition::AdditiveMigration); } #[test] fn required_missing_is_unknown() { let mut value = request(); value.required_claim_ids.push("claim:missing".into()); assert!(model_local_evidence_surveillance_contract(&value).unwrap().unknown_order.contains(&"claim:missing".to_string())); } #[test] fn unknown_is_preserved() { let mut value = request(); value.claims[0].evidence_state = EvidenceState::Unknown; assert!(model_local_evidence_surveillance_contract(&value).unwrap().semantic_loss.iter().any(|item| item.contains("unknown-not-asserted"))); } #[test] fn breaking_schema_is_partial() { let mut value = request(); value.input_schema = "EvidenceFeed9@1".into(); assert_eq!(model_local_evidence_surveillance_contract(&value).unwrap().compatibility, ContractCompatibilityDisposition::Breaking); } #[test] fn policy_blocks() { let mut value = request(); value.policy_allow = false; assert_eq!(model_local_evidence_surveillance_contract(&value).unwrap().effect_receipts, vec!["block:unsafe-release"]); } #[test] fn digest_is_stable() { let first = model_local_evidence_surveillance_contract(&request()).unwrap(); let second = model_local_evidence_surveillance_contract(&request()).unwrap(); assert_eq!(first.digest().unwrap(), second.digest().unwrap()); } }
