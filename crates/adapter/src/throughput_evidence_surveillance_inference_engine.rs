//! Prospective high-throughput evidence-surveillance admission engine.
//!
//! Atlas feature: `AFA-adapter-P01-F03`. Bounded admission and checkpoint witnesses make queue
//! pressure observable; an overflow is never silently treated as absent evidence.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceAvailability, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F03";
pub const CONTRACT_VERSION: &str = "adapter-throughput-evidence-surveillance-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceObservation {
    pub observation_id: String,
    pub batch_id: String,
    pub sequence: u64,
    pub study_id: String,
    pub modality: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceRequest {
    pub request_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub previous_checkpoint: Option<ContentHash>,
    pub observations: Vec<ThroughputEvidenceObservation>,
    pub max_items: usize,
    pub budget_units: usize,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputEvidenceSurveillanceDisposition { Completed, Partial, Unknown, Blocked }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub disposition: ThroughputEvidenceSurveillanceDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: ThroughputQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputEvidenceSurveillanceError { #[error("invalid throughput evidence surveillance request: {0}")] Invalid(String), #[error("throughput evidence artifact failed: {0}")] Artifact(String) }

fn sorted_unique(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }

impl ThroughputEvidenceSurveillanceReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local || self.request_id.trim().is_empty() || self.batch_id.trim().is_empty() || self.checkpoint_seq == 0 || self.candidate_order.is_empty() || self.effect_receipts.is_empty() || self.qualified_set.batch_id != self.batch_id { return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput identity, checkpoint, locality, candidates, effects, or qualified-set linkage is incomplete".into())); }
        for values in [&self.candidate_order, &self.selected_order, &self.unresolved_order, &self.denied_order, &self.overflow_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts, &self.qualified_set.selected_order, &self.qualified_set.omissions, &self.qualified_set.uncertainty, &self.qualified_set.negative_order] { if !sorted_unique(values) { return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput ordering is not canonical".into())); } }
        if self.ranked_order.len() != self.candidate_order.len() || self.ranked_order.iter().collect::<BTreeSet<_>>() != self.candidate_order.iter().collect::<BTreeSet<_>>() { return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput ranking must cover candidates exactly".into())); }
        let classified = self.selected_order.iter().chain(self.unresolved_order.iter()).chain(self.denied_order.iter()).chain(self.overflow_order.iter()).cloned().collect::<BTreeSet<_>>(); if classified.len() != self.candidate_order.len() || classified.iter().any(|item| !self.candidate_order.contains(item)) || self.qualified_set.selected_order != self.selected_order { return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput states do not partition candidates".into())); }
        for digest in [&self.queue_digest, &self.checkpoint_digest, &self.evidence_digest, &self.provenance_digest, &self.replay_identity, &self.artifact.content_hash] { if digest.as_str().len() != 64 { return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput digest is invalid".into())); } }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("read:local-throughput-evidence:") && effect != "block:unsafe-release") { return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput effect is outside local-read gate".into())); }
        if self.disposition == ThroughputEvidenceSurveillanceDisposition::Blocked && self.effect_receipts != vec!["block:unsafe-release".to_string()] { return Err(ThroughputEvidenceSurveillanceError::Invalid("blocked throughput surveillance must be explicitly blocked".into())); }
        self.artifact.validate_metadata().map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))
    }
}

pub fn throughput_evidence_surveillance_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["AURORA extension developer".into(), "queue operator".into()].into(), behavior: "deterministically admits bounded prospective evidence batches with checkpoint and capacity witnesses".into(), value: "preserves high-throughput omissions and overflow instead of silently dropping evidence".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "OpenTelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_throughput_evidence_surveillance(request: &ThroughputEvidenceSurveillanceRequest) -> Result<ThroughputEvidenceSurveillanceReceipt, ThroughputEvidenceSurveillanceError> {
    if request.request_id.trim().is_empty() || request.batch_id.trim().is_empty() || request.checkpoint_seq == 0 || request.observations.is_empty() || request.max_items == 0 || request.budget_units == 0 || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local || request.replay_identity.as_str().len() != 64 { return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput identity, checkpoint, observations, capacity, budget, replay, locality, or boundary is invalid".into())); }
    if request.observations.iter().any(|item| item.batch_id != request.batch_id || item.observation_id.trim().is_empty()) { return Err(ThroughputEvidenceSurveillanceError::Invalid("observation batch or identity mismatch".into())); }
    let mut observations = request.observations.clone(); observations.sort_by(|left, right| left.sequence.cmp(&right.sequence).then_with(|| left.observation_id.cmp(&right.observation_id))); let key = |item: &ThroughputEvidenceObservation| item.observation_id.clone(); let ranked_order = observations.iter().map(key).collect::<Vec<_>>(); let mut candidate_order = ranked_order.clone(); candidate_order.sort(); if candidate_order.windows(2).any(|pair| pair[0] == pair[1]) { return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput observation identities must be unique".into())); }
    let admission_limit = request.max_items.min(request.budget_units); let (admitted, overflow) = observations.split_at(admission_limit.min(observations.len())); let overflow_order = overflow.iter().map(key).collect::<BTreeSet<_>>(); let mut selected = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut denied = BTreeSet::new(); let mut digest_map = BTreeMap::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    if observations.len() > request.max_items { omissions.insert(format!("queue:capacity-exceeded:{}", observations.len() - request.max_items)); }
    if request.budget_units < request.max_items { omissions.insert(format!("queue:budget-bounded:{}", request.max_items - request.budget_units)); }
    for item in admitted { let item_key = key(item); if !request.policy_allow || !request.protected_closure || !request.raw_data_local { denied.insert(item_key.clone()); omissions.insert(format!("evidence:{}:policy-closure-locality", item_key)); } else if item.availability != EvidenceAvailability::Available { unresolved.insert(item_key.clone()); omissions.insert(format!("evidence:{}:availability-{:?}", item_key, item.availability)); } else if item.relevance_score < request.min_relevance_score { unresolved.insert(item_key.clone()); uncertainty.insert(format!("evidence:{}:relevance-below-threshold", item_key)); } else if item.digest.is_none() { unresolved.insert(item_key.clone()); omissions.insert(format!("evidence:{}:content-digest-missing", item_key)); } else if matches!(item.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative) { unresolved.insert(item_key.clone()); uncertainty.insert(format!("evidence:{}:unknown-not-asserted", item_key)); } else if item.evidence_state == EvidenceState::Contradicted { denied.insert(item_key.clone()); negative.insert(format!("evidence:{}:contradicted", item_key)); } else { selected.insert(item_key.clone()); digest_map.insert(item_key.clone(), item.digest.clone().expect("digest checked")); if item.negative_result { negative.insert(format!("evidence:{}:negative-result", item_key)); } } }
    if !request.policy_allow { omissions.insert("control:policy-denied".into()); } if !request.protected_closure { omissions.insert("control:protected-closure-incomplete".into()); } if !request.raw_data_local { omissions.insert("control:raw-data-locality-failed".into()); }
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { ThroughputEvidenceSurveillanceDisposition::Blocked } else if selected.is_empty() { ThroughputEvidenceSurveillanceDisposition::Unknown } else if !unresolved.is_empty() || !denied.is_empty() || !overflow_order.is_empty() { ThroughputEvidenceSurveillanceDisposition::Partial } else { ThroughputEvidenceSurveillanceDisposition::Completed };
    let selected_order = selected.iter().cloned().collect::<Vec<_>>(); let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>(); let denied_order = denied.iter().cloned().collect::<Vec<_>>(); let overflow_order = overflow_order.into_iter().collect::<Vec<_>>(); let omissions_vec = omissions.iter().cloned().collect::<Vec<_>>(); let uncertainty_vec = uncertainty.iter().cloned().collect::<Vec<_>>(); let negative_vec = negative.iter().cloned().collect::<Vec<_>>(); let queue_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "candidate_order": candidate_order.clone(), "ranked_order": ranked_order.clone(), "overflow_order": overflow_order.clone()})).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?; let checkpoint_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "checkpoint_seq": request.checkpoint_seq, "previous_checkpoint": request.previous_checkpoint, "queue_digest": queue_digest})).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?; let evidence_digest = ContentHash::of_value(&json!({"selected_order": selected_order.clone(), "unresolved_order": unresolved_order.clone(), "denied_order": denied_order.clone()})).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?; let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "checkpoint_digest": checkpoint_digest, "evidence_digest": evidence_digest})).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?; let selected_digests = selected_order.iter().filter_map(|item| digest_map.get(item).cloned()).collect::<Vec<_>>(); let qualified_set = ThroughputQualifiedEvidenceSet { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), set_id: format!("qualified-throughput-evidence:{}", request.batch_id), batch_id: request.batch_id.clone(), checkpoint_seq: request.checkpoint_seq, selected_order: selected_order.clone(), selected_digests, omissions: omissions_vec.clone(), uncertainty: uncertainty_vec.clone(), negative_order: negative_vec.clone(), boundary: PRECLINICAL_BOUNDARY.into() }; let payload = serde_json::to_value(&qualified_set).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?; let artifact = TypedResearchArtifact::from_payload(qualified_set.set_id.clone(), "application/vnd.aurora.qualified-throughput-evidence-set+json", &payload, Vec::new(), Vec::new()).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?; let receipt = ThroughputEvidenceSurveillanceReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), batch_id: request.batch_id.clone(), checkpoint_seq: request.checkpoint_seq, disposition, candidate_order, ranked_order, selected_order, unresolved_order, denied_order, overflow_order, queue_digest, checkpoint_digest, evidence_digest, provenance_digest, replay_identity: request.replay_identity.clone(), omissions: omissions_vec, uncertainty: uncertainty_vec, negative_evidence: negative_vec, effect_receipts: if disposition == ThroughputEvidenceSurveillanceDisposition::Blocked { vec!["block:unsafe-release".into()] } else { vec![format!("read:local-throughput-evidence:{}", request.batch_id)] }, qualified_set, artifact, raw_data_local: request.raw_data_local, boundary: PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> ThroughputEvidenceSurveillanceRequest { let digest = hash("throughput-evidence"); let observation = |id: &str, sequence: u64, state: EvidenceState| ThroughputEvidenceObservation { observation_id: id.into(), batch_id: "batch:one".into(), sequence, study_id: "study:one".into(), modality: "imaging".into(), digest: Some(digest.clone()), availability: EvidenceAvailability::Available, evidence_state: state, relevance_score: 90, negative_result: id == "negative" }; ThroughputEvidenceSurveillanceRequest { request_id: "request:throughput".into(), batch_id: "batch:one".into(), checkpoint_seq: 7, previous_checkpoint: Some(digest.clone()), observations: vec![observation("obs:a", 1, EvidenceState::Supported), observation("negative", 2, EvidenceState::Supported)], max_items: 4, budget_units: 4, min_relevance_score: 70, policy_allow: true, protected_closure: true, raw_data_local: true, replay_identity: digest, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(throughput_evidence_surveillance_inference_engine_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn bounded_batch_completes() { let receipt = run_throughput_evidence_surveillance(&request()).unwrap(); assert_eq!(receipt.disposition, ThroughputEvidenceSurveillanceDisposition::Completed); }
    #[test] fn overflow_is_partial() { let mut value = request(); value.max_items = 1; let receipt = run_throughput_evidence_surveillance(&value).unwrap(); assert_eq!(receipt.disposition, ThroughputEvidenceSurveillanceDisposition::Partial); assert_eq!(receipt.overflow_order.len(), 1); }
    #[test] fn missing_digest_is_unresolved() { let mut value = request(); value.observations[0].digest = None; let receipt = run_throughput_evidence_surveillance(&value).unwrap(); assert!(receipt.unresolved_order.contains(&"obs:a".to_string())); }
    #[test] fn unknown_is_not_asserted() { let mut value = request(); value.observations[0].evidence_state = EvidenceState::Unknown; let receipt = run_throughput_evidence_surveillance(&value).unwrap(); assert!(receipt.uncertainty.iter().any(|item| item.contains("unknown-not-asserted"))); }
    #[test] fn policy_blocks() { let mut value = request(); value.policy_allow = false; let receipt = run_throughput_evidence_surveillance(&value).unwrap(); assert_eq!(receipt.disposition, ThroughputEvidenceSurveillanceDisposition::Blocked); }
    #[test] fn checkpoint_digest_is_stable() { let first = run_throughput_evidence_surveillance(&request()).unwrap(); let second = run_throughput_evidence_surveillance(&request()).unwrap(); assert_eq!(first.checkpoint_digest, second.checkpoint_digest); }
}
