//! Multimodal multi-study evidence-surveillance inference engine.
//!
//! Atlas feature: `AFA-adapter-P01-F02`. This A1 scientific algorithm makes study × modality
//! comparability an explicit gate: it never treats one complete modality as a complete study.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F02";
pub const CONTRACT_VERSION: &str = "adapter-multimodal-evidence-surveillance-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed2@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceObservation {
    pub source_id: String,
    pub study_id: String,
    pub modality: String,
    pub source_type: String,
    pub locator: String,
    pub semantic_profile: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceRequest {
    pub request_id: String,
    pub intent: String,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub semantic_profile: String,
    pub observations: Vec<MultimodalEvidenceObservation>,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalEvidenceSurveillanceDisposition { Completed, Partial, Unknown, Blocked }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub intent: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub coverage_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub intent: String,
    pub disposition: MultimodalEvidenceSurveillanceDisposition,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: MultimodalQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalEvidenceSurveillanceError {
    #[error("invalid multimodal evidence surveillance request: {0}")] Invalid(String),
    #[error("multimodal evidence surveillance artifact failed: {0}")] Artifact(String),
}

fn sorted_unique(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }

impl MultimodalEvidenceSurveillanceReceipt {
    pub fn validate(&self) -> Result<(), MultimodalEvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local || self.request_id.trim().is_empty() || self.intent.trim().is_empty() || self.study_order.len() < 2 || self.modality_order.len() < 2 || self.candidate_order.is_empty() || self.effect_receipts.is_empty() || self.qualified_set.intent != self.intent { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal identity, study/modality closure, locality, candidates, effects, or qualified-set linkage is incomplete".into())); }
        for values in [&self.study_order, &self.modality_order, &self.candidate_order, &self.selected_order, &self.unresolved_order, &self.denied_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts, &self.qualified_set.study_order, &self.qualified_set.modality_order, &self.qualified_set.selected_order, &self.qualified_set.coverage_order, &self.qualified_set.omissions, &self.qualified_set.uncertainty, &self.qualified_set.negative_order] { if !sorted_unique(values) { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal ordering is not canonical".into())); } }
        if self.ranked_order.len() != self.candidate_order.len() || self.ranked_order.iter().collect::<BTreeSet<_>>() != self.candidate_order.iter().collect::<BTreeSet<_>>() { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal ranking must cover candidates exactly".into())); }
        let classified = self.selected_order.iter().chain(self.unresolved_order.iter()).chain(self.denied_order.iter()).cloned().collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len() || classified.iter().any(|key| !self.candidate_order.contains(key)) || self.qualified_set.selected_order != self.selected_order { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal states do not partition candidates".into())); }
        for digest in [&self.comparability_digest, &self.evidence_digest, &self.provenance_digest, &self.replay_identity, &self.artifact.content_hash] { if digest.as_str().len() != 64 { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal digest is invalid".into())); } }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("read:local-multimodal-evidence:") && effect != "block:unsafe-release") { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal effect is outside local-read gate".into())); }
        if self.disposition == MultimodalEvidenceSurveillanceDisposition::Blocked && self.effect_receipts != vec!["block:unsafe-release".to_string()] { return Err(MultimodalEvidenceSurveillanceError::Invalid("blocked multimodal surveillance must be explicitly blocked".into())); }
        self.artifact.validate_metadata().map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))
    }
}

pub fn multimodal_evidence_surveillance_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["integration engineer".into(), "multimodal evidence steward".into()].into(), behavior: "deterministically computes a comparable study-by-modality evidence stream while retaining incomplete cells and negative results".into(), value: "prevents single-modality evidence from being promoted as complete multi-study discovery".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "OME-NGFF".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }, EvidenceReference { source_id: "AnnData".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_multimodal_evidence_surveillance(request: &MultimodalEvidenceSurveillanceRequest) -> Result<MultimodalEvidenceSurveillanceReceipt, MultimodalEvidenceSurveillanceError> {
    if request.request_id.trim().is_empty() || request.intent.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.required_studies.len() < 2 || request.required_modalities.len() < 2 || request.observations.is_empty() || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local || request.replay_identity.as_str().len() != 64 { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal identity, required study/modality closure, observations, replay, locality, or boundary is invalid".into())); }
    let mut studies = request.required_studies.clone(); studies.sort(); studies.dedup(); let mut modalities = request.required_modalities.clone(); modalities.sort(); modalities.dedup(); if studies.len() < 2 || modalities.len() < 2 || !modalities.iter().any(|value| value == "imaging") || !modalities.iter().any(|value| value == "omics") { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal request must declare at least two studies and imaging plus omics modalities".into())); }
    let mut observations = request.observations.clone(); observations.sort_by(|left, right| right.relevance_score.cmp(&left.relevance_score).then_with(|| left.study_id.cmp(&right.study_id)).then_with(|| left.modality.cmp(&right.modality)).then_with(|| left.source_id.cmp(&right.source_id)));
    let key = |observation: &MultimodalEvidenceObservation| format!("{}::{}::{}", observation.study_id, observation.modality, observation.source_id);
    let ranked_order = observations.iter().map(key).collect::<Vec<_>>(); let mut candidate_order = ranked_order.clone(); candidate_order.sort(); if candidate_order.windows(2).any(|pair| pair[0] == pair[1] || pair[0] > pair[1]) { return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal observation keys must be unique and non-empty".into())); }
    let required_cells = studies.iter().flat_map(|study| modalities.iter().map(move |modality| format!("{}::{}", study, modality))).collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut denied = BTreeSet::new(); let mut digest_map = BTreeMap::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new(); let mut selected_cells = BTreeSet::new();
    for observation in &observations {
        let item_key = key(observation); let cell = format!("{}::{}", observation.study_id, observation.modality);
        if !studies.contains(&observation.study_id) || !modalities.contains(&observation.modality) || observation.source_type.trim().is_empty() || observation.locator.trim().is_empty() || !request.policy_allow || !request.protected_closure || !request.raw_data_local { denied.insert(item_key.clone()); omissions.insert(format!("evidence:{}:scope-policy-locality", item_key)); }
        else if observation.semantic_profile != request.semantic_profile { denied.insert(item_key.clone()); omissions.insert(format!("evidence:{}:semantic-profile-mismatch", item_key)); negative.insert(format!("evidence:{}:incomparable", item_key)); }
        else if observation.availability != EvidenceAvailability::Available { unresolved.insert(item_key.clone()); omissions.insert(format!("evidence:{}:availability-{:?}", item_key, observation.availability)); }
        else if observation.relevance_score < request.min_relevance_score { unresolved.insert(item_key.clone()); uncertainty.insert(format!("evidence:{}:relevance-below-threshold", item_key)); }
        else if observation.digest.is_none() { unresolved.insert(item_key.clone()); omissions.insert(format!("evidence:{}:content-digest-missing", item_key)); }
        else if matches!(observation.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative) { unresolved.insert(item_key.clone()); uncertainty.insert(format!("evidence:{}:unknown-not-asserted", item_key)); }
        else if observation.evidence_state == EvidenceState::Contradicted { denied.insert(item_key.clone()); negative.insert(format!("evidence:{}:contradicted", item_key)); }
        else { selected.insert(item_key.clone()); selected_cells.insert(cell); digest_map.insert(item_key, observation.digest.clone().expect("digest checked")); if observation.negative_result { negative.insert(format!("evidence:{}:negative-result", key(observation))); } }
    }
    for cell in &required_cells { if !selected_cells.contains(cell) { omissions.insert(format!("cell:{}:required-modality-study-missing", cell)); uncertainty.insert(format!("cell:{}:comparability-incomplete", cell)); } }
    if !request.policy_allow { omissions.insert("control:policy-denied".into()); } if !request.protected_closure { omissions.insert("control:protected-closure-incomplete".into()); } if !request.raw_data_local { omissions.insert("control:raw-data-locality-failed".into()); }
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { MultimodalEvidenceSurveillanceDisposition::Blocked } else if selected.is_empty() { MultimodalEvidenceSurveillanceDisposition::Unknown } else if !unresolved.is_empty() || !denied.is_empty() || selected_cells != required_cells { MultimodalEvidenceSurveillanceDisposition::Partial } else { MultimodalEvidenceSurveillanceDisposition::Completed };
    let selected_order = selected.iter().cloned().collect::<Vec<_>>(); let selected_digests = selected_order.iter().filter_map(|item| digest_map.get(item).cloned()).collect::<Vec<_>>(); let coverage_order = selected_cells.iter().cloned().collect::<Vec<_>>(); let omissions_vec = omissions.iter().cloned().collect::<Vec<_>>(); let uncertainty_vec = uncertainty.iter().cloned().collect::<Vec<_>>(); let negative_vec = negative.iter().cloned().collect::<Vec<_>>(); let comparability_digest = ContentHash::of_value(&json!({"study_order": studies.clone(), "modality_order": modalities.clone(), "semantic_profile": request.semantic_profile, "coverage_order": coverage_order.clone()})).map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?; let evidence_digest = ContentHash::of_value(&json!({"candidate_order": candidate_order.clone(), "ranked_order": ranked_order.clone(), "selected_order": selected_order.clone(), "unresolved_order": unresolved.clone(), "denied_order": denied.clone()})).map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?; let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "comparability_digest": comparability_digest, "evidence_digest": evidence_digest})).map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?; let qualified_set = MultimodalQualifiedEvidenceSet { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), set_id: format!("qualified-multimodal-evidence:{}", request.request_id), intent: request.intent.clone(), study_order: studies.clone(), modality_order: modalities.clone(), selected_order: selected_order.clone(), selected_digests, coverage_order, omissions: omissions_vec.clone(), uncertainty: uncertainty_vec.clone(), negative_order: negative_vec.clone(), semantic_profile: request.semantic_profile.clone(), evidence_state: if disposition == MultimodalEvidenceSurveillanceDisposition::Completed { EvidenceState::Supported } else { EvidenceState::Unknown }, boundary: PRECLINICAL_BOUNDARY.into() }; let payload = serde_json::to_value(&qualified_set).map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?; let artifact = TypedResearchArtifact::from_payload(qualified_set.set_id.clone(), "application/vnd.aurora.qualified-multimodal-evidence-set+json", &payload, Vec::new(), Vec::new()).map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?; let receipt = MultimodalEvidenceSurveillanceReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), intent: request.intent.clone(), disposition, study_order: studies, modality_order: modalities, candidate_order, ranked_order, selected_order, unresolved_order: unresolved.into_iter().collect(), denied_order: denied.into_iter().collect(), comparability_digest, evidence_digest, provenance_digest, replay_identity: request.replay_identity.clone(), omissions: omissions_vec, uncertainty: uncertainty_vec, negative_evidence: negative_vec, effect_receipts: if disposition == MultimodalEvidenceSurveillanceDisposition::Blocked { vec!["block:unsafe-release".into()] } else { vec![format!("read:local-multimodal-evidence:{}", request.request_id)] }, qualified_set, artifact, raw_data_local: request.raw_data_local, boundary: PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> MultimodalEvidenceSurveillanceRequest { let digest = hash("multimodal-evidence"); let observation = |study: &str, modality: &str, source: &str, score: u16, state: EvidenceState| MultimodalEvidenceObservation { source_id: source.into(), study_id: study.into(), modality: modality.into(), source_type: "local-record".into(), locator: format!("local://{study}/{modality}/{source}"), semantic_profile: "profile:ome-ngff-anndata-v1".into(), digest: Some(digest.clone()), availability: EvidenceAvailability::Available, evidence_state: state, relevance_score: score, negative_result: source == "negative" }; MultimodalEvidenceSurveillanceRequest { request_id: "request:multimodal-evidence".into(), intent: "compare mechanism evidence".into(), required_studies: vec!["study:a".into(), "study:b".into()], required_modalities: vec!["imaging".into(), "omics".into()], semantic_profile: "profile:ome-ngff-anndata-v1".into(), observations: vec![observation("study:a", "imaging", "source:a-image", 95, EvidenceState::Supported), observation("study:a", "omics", "source:a-omics", 90, EvidenceState::Supported), observation("study:b", "imaging", "source:b-image", 85, EvidenceState::Supported), observation("study:b", "omics", "negative", 80, EvidenceState::Supported)], min_relevance_score: 70, policy_allow: true, protected_closure: true, raw_data_local: true, replay_identity: digest, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(multimodal_evidence_surveillance_inference_engine_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn comparable_cells_complete() { let receipt = run_multimodal_evidence_surveillance(&request()).unwrap(); assert_eq!(receipt.disposition, MultimodalEvidenceSurveillanceDisposition::Completed); assert!(receipt.negative_evidence.iter().any(|value| value.contains("negative-result"))); }
    #[test] fn missing_modality_is_partial() { let mut value = request(); value.observations.pop(); let receipt = run_multimodal_evidence_surveillance(&value).unwrap(); assert_eq!(receipt.disposition, MultimodalEvidenceSurveillanceDisposition::Partial); assert!(receipt.omissions.iter().any(|item| item.contains("study:b::omics"))); }
    #[test] fn semantic_profile_is_incomparable() { let mut value = request(); value.observations[0].semantic_profile = "profile:other".into(); let receipt = run_multimodal_evidence_surveillance(&value).unwrap(); assert!(receipt.denied_order.iter().any(|item| item.contains("study:a::imaging"))); }
    #[test] fn unknown_is_not_asserted() { let mut value = request(); value.observations[0].evidence_state = EvidenceState::Unknown; let receipt = run_multimodal_evidence_surveillance(&value).unwrap(); assert!(receipt.uncertainty.iter().any(|item| item.contains("unknown-not-asserted"))); }
    #[test] fn policy_blocks() { let mut value = request(); value.policy_allow = false; let receipt = run_multimodal_evidence_surveillance(&value).unwrap(); assert_eq!(receipt.disposition, MultimodalEvidenceSurveillanceDisposition::Blocked); assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]); }
    #[test] fn replay_digest_is_stable() { let receipt = run_multimodal_evidence_surveillance(&request()).unwrap(); let first = serde_json::to_value(&receipt).unwrap(); let second = serde_json::to_value(&run_multimodal_evidence_surveillance(&request()).unwrap()).unwrap(); assert_eq!(first, second); }
}
