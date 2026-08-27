//! Prospective high-throughput evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-ids-P01-F07`.  This is an ids-owned typed data primitive:
//! it admits a bounded EvidenceFeed3 batch, binds schema/checkpoint/replay
//! identity, and retains overflow, omission, uncertainty, and negative-result
//! witnesses instead of silently dropping claims.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P01-F07";
pub const CONTRACT_VERSION: &str = "ids-prospective-throughput-evidence-surveillance-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";
pub const RESEARCH_CONTRACT_SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.qualified-throughput-evidence-set+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState { Proven, Supported, Unknown, Speculative, Contradicted, Unmeasured }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractClaim {
    pub claim_id: String,
    pub sequence: u64,
    pub semantic_type: String,
    pub value_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub omitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeedRequest {
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub previous_checkpoint: Option<ContentHash>,
    pub max_claims: usize,
    pub budget_units: usize,
    pub claims: Vec<ContractClaim>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedArtifact {
    pub schema_version: String,
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance: Vec<String>,
    pub boundary: String,
}

impl TypedArtifact {
    fn validate(&self) -> Result<(), ContractModelError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.artifact_id.trim().is_empty()
            || self.content_type != CONTENT_TYPE
            || self.content_hash.as_str().len() != 64
            || self.boundary != PRECLINICAL_BOUNDARY
        { return Err(ContractModelError::Artifact("typed evidence artifact metadata is incomplete".into())); }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractDisposition { Compatible, Partial, Unknown, Blocked }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSurveillanceContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub disposition: ContractDisposition,
    pub candidate_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub semantic_loss: Vec<String>,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub contract_digest: ContentHash,
    pub canonical_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractModelError {
    #[error("invalid ids evidence contract request: {0}")] Invalid(String),
    #[error("ids evidence contract artifact failed: {0}")] Artifact(String),
}

fn canonical(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 }

impl EvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), ContractModelError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
        { return Err(ContractModelError::Invalid("ids evidence contract identity, schemas, checkpoint, locality, candidates, or effects are incomplete".into())); }
        for values in [&self.candidate_order, &self.retained_order, &self.unknown_order, &self.denied_order, &self.overflow_order, &self.omission_order, &self.semantic_loss, &self.effect_receipts] {
            if !canonical(values) { return Err(ContractModelError::Invalid("ids evidence contract ordering is not canonical".into())); }
        }
        let classified = self.retained_order.iter().chain(self.unknown_order.iter()).chain(self.denied_order.iter()).chain(self.overflow_order.iter()).cloned().collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect::<BTreeSet<_>>() || classified.len() != self.candidate_order.len() {
            return Err(ContractModelError::Invalid("ids evidence contract states do not partition candidates".into()));
        }
        for value in [&self.queue_digest, &self.checkpoint_digest, &self.contract_digest, &self.canonical_digest, &self.provenance_digest, &self.replay_identity, &self.artifact.content_hash] {
            if !digest(value) { return Err(ContractModelError::Invalid("ids evidence contract digest is invalid".into())); }
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("read:local-evidence-contract:") && effect != "block:unsafe-release") {
            return Err(ContractModelError::Invalid("ids evidence contract effect is outside local-read gate".into()));
        }
        if self.disposition == ContractDisposition::Blocked && self.effect_receipts != ["block:unsafe-release"] {
            return Err(ContractModelError::Invalid("blocked ids evidence contract must explicitly block release".into()));
        }
        self.artifact.validate()
    }
}

/// Returns the stable contract metadata consumed by registry and SDK surfaces.
pub fn throughput_evidence_surveillance_contract_model_manifest() -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["context compiler engineer", "evidence schema steward"],
        "behavior": "models EvidenceFeed3 into QualifiedEvidenceSet2 with bounded admission, checkpoint, migration, and overflow witnesses",
        "value": "makes high-throughput evidence capacity loss and replay identity part of the typed scientific data contract",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["read:local-research-artifacts", "write:local-artifact"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

pub fn model_throughput_evidence_surveillance_contract(request: &EvidenceFeedRequest) -> Result<EvidenceSurveillanceContractReceipt, ContractModelError> {
    if request.request_id.trim().is_empty() || request.input_schema.trim().is_empty() || request.output_schema.trim().is_empty() || request.batch_id.trim().is_empty() || request.checkpoint_seq == 0 || request.max_claims == 0 || request.budget_units == 0 || request.claims.is_empty() || !digest(&request.replay_identity) || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local {
        return Err(ContractModelError::Invalid("ids evidence contract identity, schemas, batch/checkpoint, capacity, budget, claims, replay, locality, or boundary is invalid".into()));
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.sequence.cmp(&right.sequence).then_with(|| left.claim_id.cmp(&right.claim_id)));
    if claims.iter().any(|claim| claim.claim_id.trim().is_empty() || claim.semantic_type.trim().is_empty() || !digest(&claim.value_digest)) || claims.windows(2).any(|pair| pair[0].claim_id == pair[1].claim_id) {
        return Err(ContractModelError::Invalid("ids evidence claim identities or value digests are malformed or duplicated".into()));
    }
    let compatibility = request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA;
    let admission = request.max_claims.min(request.budget_units).min(claims.len());
    let (admitted, overflow) = claims.split_at(admission);
    let mut candidate_order = claims.iter().map(|claim| claim.claim_id.clone()).collect::<Vec<_>>(); candidate_order.sort();
    let overflow_order = overflow.iter().map(|claim| claim.claim_id.clone()).collect::<BTreeSet<_>>();
    let mut retained = BTreeSet::new(); let mut unknown = BTreeSet::new(); let mut denied = BTreeSet::new(); let mut omission = BTreeSet::new(); let mut loss = BTreeSet::new();
    if claims.len() > request.max_claims { loss.insert(format!("queue:capacity-overflow:{}", claims.len() - request.max_claims)); }
    if request.budget_units < request.max_claims { loss.insert(format!("queue:budget-bounded:{}", request.max_claims - request.budget_units)); }
    for claim in admitted {
        if !compatibility { denied.insert(claim.claim_id.clone()); loss.insert(format!("claim:{}:breaking-schema", claim.claim_id)); }
        else if claim.omitted || matches!(claim.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative | EvidenceState::Unmeasured) { unknown.insert(claim.claim_id.clone()); loss.insert(format!("claim:{}:evidence-unresolved", claim.claim_id)); }
        else if claim.evidence_state == EvidenceState::Contradicted { denied.insert(claim.claim_id.clone()); loss.insert(format!("claim:{}:contradicted-retained", claim.claim_id)); }
        else { retained.insert(claim.claim_id.clone()); if claim.negative_result { loss.insert(format!("claim:{}:negative-result-retained", claim.claim_id)); } }
    }
    if !request.policy_allow { omission.insert("control:policy-denied".into()); }
    if !request.protected_closure { omission.insert("control:protected-closure-incomplete".into()); }
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { ContractDisposition::Blocked } else if retained.is_empty() { ContractDisposition::Unknown } else if !unknown.is_empty() || !denied.is_empty() || !overflow_order.is_empty() { ContractDisposition::Partial } else { ContractDisposition::Compatible };
    let retained_order = retained.into_iter().collect::<Vec<_>>(); let unknown_order = unknown.into_iter().collect::<Vec<_>>(); let denied_order = denied.into_iter().collect::<Vec<_>>(); let overflow_order = overflow_order.into_iter().collect::<Vec<_>>(); let omission_order = omission.into_iter().collect::<Vec<_>>(); let semantic_loss = loss.into_iter().collect::<Vec<_>>();
    let queue_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "candidate_order": candidate_order.clone(), "overflow_order": overflow_order.clone()})).map_err(|e| ContractModelError::Artifact(e.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "checkpoint_seq": request.checkpoint_seq, "previous_checkpoint": request.previous_checkpoint, "queue_digest": queue_digest.clone()})).map_err(|e| ContractModelError::Artifact(e.to_string()))?;
    let contract_digest = ContentHash::of_value(&json!({"input_schema": request.input_schema, "output_schema": request.output_schema, "compatible": compatibility, "candidate_order": candidate_order.clone()})).map_err(|e| ContractModelError::Artifact(e.to_string()))?;
    let canonical_digest = ContentHash::of_value(&json!({"retained_order": retained_order.clone(), "unknown_order": unknown_order.clone(), "denied_order": denied_order.clone(), "overflow_order": overflow_order.clone(), "omission_order": omission_order.clone(), "semantic_loss": semantic_loss.clone()})).map_err(|e| ContractModelError::Artifact(e.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "checkpoint_digest": checkpoint_digest.clone(), "contract_digest": contract_digest.clone()})).map_err(|e| ContractModelError::Artifact(e.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "input_schema": request.input_schema, "output_schema": request.output_schema, "batch_id": request.batch_id, "checkpoint_seq": request.checkpoint_seq, "disposition": disposition, "candidate_order": candidate_order.clone(), "retained_order": retained_order.clone(), "unknown_order": unknown_order.clone(), "denied_order": denied_order.clone(), "overflow_order": overflow_order.clone(), "omission_order": omission_order.clone(), "semantic_loss": semantic_loss.clone(), "queue_digest": queue_digest.clone(), "checkpoint_digest": checkpoint_digest.clone(), "contract_digest": contract_digest.clone(), "canonical_digest": canonical_digest.clone(), "provenance_digest": provenance_digest.clone(), "replay_identity": request.replay_identity, "raw_data_local": request.raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact_hash = ContentHash::of_value(&payload).map_err(|e| ContractModelError::Artifact(e.to_string()))?;
    let artifact = TypedArtifact { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), artifact_id: format!("ids-throughput-evidence-contract:{}", request.request_id), content_type: CONTENT_TYPE.into(), content_hash: artifact_hash, semantic_loss: Vec::new(), provenance: vec![provenance_digest.to_string()], boundary: PRECLINICAL_BOUNDARY.into() };
    let effect_receipts = if disposition == ContractDisposition::Blocked { vec!["block:unsafe-release".into()] } else { vec![format!("read:local-evidence-contract:{}", request.request_id)] };
    let receipt = EvidenceSurveillanceContractReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), input_schema: INPUT_SCHEMA.into(), output_schema: OUTPUT_SCHEMA.into(), batch_id: request.batch_id.clone(), checkpoint_seq: request.checkpoint_seq, disposition, candidate_order, retained_order, unknown_order, denied_order, overflow_order, omission_order, semantic_loss, queue_digest, checkpoint_digest, contract_digest, canonical_digest, provenance_digest, replay_identity: request.replay_identity.clone(), effect_receipts, artifact, raw_data_local: request.raw_data_local, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> EvidenceFeedRequest {
        let digest = hash("ids-throughput");
        let claim = |id: &str, seq: u64, state: EvidenceState| ContractClaim { claim_id: id.into(), sequence: seq, semantic_type: "evidence".into(), value_digest: digest.clone(), evidence_state: state, omitted: false, negative_result: false };
        EvidenceFeedRequest { request_id: "request:ids-throughput".into(), input_schema: INPUT_SCHEMA.into(), output_schema: OUTPUT_SCHEMA.into(), batch_id: "batch:one".into(), checkpoint_seq: 1, previous_checkpoint: None, max_claims: 4, budget_units: 4, claims: vec![claim("claim:a", 1, EvidenceState::Supported), claim("claim:b", 2, EvidenceState::Supported)], policy_allow: true, protected_closure: true, raw_data_local: true, replay_identity: digest, boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn manifest_is_a1() { assert_eq!(throughput_evidence_surveillance_contract_model_manifest()["autonomy_tier"], "A1"); }
    #[test] fn compatible_admission() { assert_eq!(model_throughput_evidence_surveillance_contract(&request()).unwrap().disposition, ContractDisposition::Compatible); }
    #[test] fn overflow_is_partial() { let mut r = request(); r.max_claims = 1; assert_eq!(model_throughput_evidence_surveillance_contract(&r).unwrap().disposition, ContractDisposition::Partial); }
    #[test] fn unknown_is_retained() { let mut r = request(); r.claims[0].evidence_state = EvidenceState::Unknown; let out = model_throughput_evidence_surveillance_contract(&r).unwrap(); assert!(!out.unknown_order.is_empty()); }
    #[test] fn contradiction_is_denied() { let mut r = request(); r.claims[0].evidence_state = EvidenceState::Contradicted; let out = model_throughput_evidence_surveillance_contract(&r).unwrap(); assert!(!out.denied_order.is_empty()); }
    #[test] fn policy_blocks() { let mut r = request(); r.policy_allow = false; assert_eq!(model_throughput_evidence_surveillance_contract(&r).unwrap().effect_receipts, vec!["block:unsafe-release"]); }
    #[test] fn replay_is_stable() { let a = model_throughput_evidence_surveillance_contract(&request()).unwrap(); let b = model_throughput_evidence_surveillance_contract(&request()).unwrap(); assert_eq!(a.provenance_digest, b.provenance_digest); }
}
