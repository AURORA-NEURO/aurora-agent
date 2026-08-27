//! Federated continual statistical, causal, and ML analysis assurance.
//!
//! This module verifies analysis attestations and release predicates. It never runs a model or
//! treats a metric as a scientific conclusion; institution-local data and model payloads stay
//! behind the digest-only boundary.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface,
    SemanticLoss, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ops-P13-F28";
pub const CONTRACT_VERSION: &str = "ops-federated-continual-analysis-assurance/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisQuestion4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedAnalysisResult7@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate {
    pub analysis_id: String,
    pub origin: String,
    pub scope: String,
    pub semantic_profile: String,
    pub estimand: String,
    pub metric_digest: ContentHash,
    pub data_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub baseline_delta_milli: i64,
    pub uncertainty_width_milli: u64,
    pub independent_site_count: u32,
    pub required_site_quorum: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAnalysisRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_origin_quorum: u32,
    pub capacity: u32,
    pub active_runs: u32,
    pub checkpoint_seq: u64,
    pub candidates: Vec<AnalysisCandidate>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub network_permitted: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisDecision {
    pub analysis_id: String,
    pub origin: String,
    pub score_milli: i64,
    pub disposition: String,
    pub failed_gates: Vec<String>,
    pub conditional_gates: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisAdmission {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAnalysisReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub admission: AnalysisAdmission,
    pub origin_order: Vec<String>,
    pub analysis_order: Vec<String>,
    pub rank_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub decisions: Vec<AnalysisDecision>,
    pub checkpoint_seq: u64,
    pub checkpoint_digest: ContentHash,
    pub control_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedAnalysisError {
    #[error("invalid federated analysis request: {0}")]
    Invalid(String),
    #[error("federated analysis artifact failed: {0}")]
    Artifact(String),
}

impl FederatedAnalysisReceipt {
    pub fn validate(&self) -> Result<(), FederatedAnalysisError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.analysis_order.is_empty()
            || self.decisions.len() != self.analysis_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedAnalysisError::Invalid("analysis identity, locality, candidates, decisions, or effects are incomplete".into()));
        }
        for values in [&self.origin_order, &self.analysis_order, &self.qualified_order, &self.unresolved_order, &self.blocked_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if values.windows(2).any(|window| window[0] >= window[1]) { return Err(FederatedAnalysisError::Invalid("analysis ordering is not canonical".into())); }
        }
        if self.rank_order.len() != self.analysis_order.len() || BTreeSet::from_iter(self.rank_order.iter().cloned()) != BTreeSet::from_iter(self.analysis_order.iter().cloned()) || self.decisions.iter().zip(&self.analysis_order).any(|(decision, id)| &decision.analysis_id != id) { return Err(FederatedAnalysisError::Invalid("analysis ranking or decisions do not match candidates".into())); }
        if BTreeSet::from_iter(self.qualified_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).cloned()) != BTreeSet::from_iter(self.analysis_order.iter().cloned()) { return Err(FederatedAnalysisError::Invalid("analysis dispositions do not partition candidates".into())); }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("qualify:analysis:") && effect != "block:unsafe-release") { return Err(FederatedAnalysisError::Invalid("analysis effect is outside the release gate".into())); }
        self.artifact.validate_metadata().map_err(|error| FederatedAnalysisError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "ops".into(), consumers: BTreeSet::from(["consortium operator".into(), "independent analysis reviewer".into()]), behavior: "verifies federated analysis attestations under evidence, baseline, site-quorum, provenance, policy, and release gates".into(), value: "prevents unsupported statistical, causal, or ML claims from entering a research release while retaining negative evidence".into(), inputs: vec![TypedPort { name: "analysis_question".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_analysis_result".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]), permissions: BTreeSet::from(["evaluate:capability-runs".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: vec![AuthorityRequirement { role: "analysis-release-reviewer".into(), reason: "qualified analysis release verdict".into() }], autonomy_tier: AutonomyTier::A1, surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure(request: &FederatedAnalysisRequest) -> Result<FederatedAnalysisReceipt, FederatedAnalysisError> {
    if request.request_id.trim().is_empty() || request.federation_id.trim().is_empty() || request.purpose.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.required_origin_quorum == 0 || request.capacity == 0 || request.active_runs > request.capacity || request.checkpoint_seq == 0 || request.candidates.is_empty() || !request.raw_data_local || request.boundary != PRECLINICAL_BOUNDARY { return Err(FederatedAnalysisError::Invalid("analysis identity, quorum, capacity, checkpoint, candidates, locality, or boundary is invalid".into())); }
    let mut candidates = request.candidates.clone(); candidates.sort_by(|a,b| a.analysis_id.cmp(&b.analysis_id)); let analysis_order = candidates.iter().map(|candidate| candidate.analysis_id.clone()).collect::<Vec<_>>();
    if analysis_order.iter().any(|id| id.trim().is_empty()) || analysis_order.windows(2).any(|window| window[0] == window[1]) { return Err(FederatedAnalysisError::Invalid("analysis identifiers must be unique and non-empty".into())); }
    let origins = candidates.iter().map(|candidate| candidate.origin.clone()).collect::<BTreeSet<_>>(); if origins.len() < request.required_origin_quorum as usize || origins.iter().any(|origin| origin.trim().is_empty()) { return Err(FederatedAnalysisError::Invalid("declared analysis origin quorum is not available".into())); }
    let mut global_failed = BTreeSet::new(); for (gate, failed) in [("policy", !request.policy_allow), ("protected-closure", !request.protected_closure), ("signed-approval", !request.signed_approval), ("network-permission", !request.network_permitted)] { if failed { global_failed.insert(gate.to_string()); } }
    let mut qualified = Vec::new(); let mut unresolved = Vec::new(); let mut blocked = Vec::new(); let mut decisions = Vec::new(); let mut semantic_loss = Vec::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new(); let mut scores = std::collections::BTreeMap::new();
    for candidate in &candidates {
        let mut failed = global_failed.clone(); let mut pending = BTreeSet::new(); let id = &candidate.analysis_id;
        if candidate.semantic_profile != request.semantic_profile { failed.insert("semantic-profile".into()); } if candidate.replay_identity != request.replay_identity { failed.insert("replay-identity".into()); } if !candidate.policy_allow { failed.insert("candidate-policy".into()); } if !candidate.protected_closure { failed.insert("candidate-protected-closure".into()); } if !candidate.signed_approval { failed.insert("candidate-signed-approval".into()); } if !candidate.raw_data_local { failed.insert("candidate-locality".into()); } if candidate.independent_site_count < candidate.required_site_quorum { pending.insert("independent-site-quorum".into()); omissions.insert(format!("{}:sites={}/{}", id, candidate.independent_site_count, candidate.required_site_quorum)); }
        let score = candidate.baseline_delta_milli - candidate.uncertainty_width_milli as i64 + candidate.independent_site_count.min(20) as i64 * 100 + if candidate.evidence_state == EvidenceState::Proven { 20000 } else if candidate.evidence_state == EvidenceState::Supported { 10000 } else { 0 }; scores.insert(id.clone(), score);
        match candidate.evidence_state { EvidenceState::Contradicted => { failed.insert("contradicted-evidence".into()); negative.insert(format!("{}:contradicted", id)); }, EvidenceState::Unknown | EvidenceState::Speculative => { pending.insert("evidence-state".into()); uncertainty.insert(format!("{}:evidence-state", id)); }, _ => {} }
        negative.insert(format!("{}:{}", id, if candidate.negative_result { "negative-result" } else { "negative-result-not-observed" })); let disposition = if !failed.is_empty() { blocked.push(id.clone()); "blocked" } else if !pending.is_empty() { unresolved.push(id.clone()); "unresolved" } else { qualified.push(id.clone()); "qualified" }; if disposition == "blocked" { semantic_loss.push(SemanticLoss { field: format!("analysis:{}", id), reason: "analysis attestation failed one or more release gates".into(), severity: LossSeverity::DecisionRelevant }); } decisions.push(AnalysisDecision { analysis_id: id.clone(), origin: candidate.origin.clone(), score_milli: score, disposition: disposition.into(), failed_gates: failed.into_iter().collect(), conditional_gates: pending.into_iter().collect(), negative_result: candidate.negative_result });
    }
    let mut rank_order = analysis_order.clone(); rank_order.sort_by(|a,b| scores[b].cmp(&scores[a]).then_with(|| a.cmp(b))); let admission = if !global_failed.is_empty() || !blocked.is_empty() { AnalysisAdmission::Blocked } else if !unresolved.is_empty() { AnalysisAdmission::Unresolved } else if qualified.is_empty() { AnalysisAdmission::Blocked } else { AnalysisAdmission::Qualified };
    let checkpoint_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "checkpoint_seq": request.checkpoint_seq, "analysis_order": analysis_order, "origin_order": origins})).map_err(|error| FederatedAnalysisError::Artifact(error.to_string()))?; let control_digest = ContentHash::of_value(&json!({"admission": admission, "rank_order": rank_order, "decisions": decisions, "semantic_loss": semantic_loss})).map_err(|error| FederatedAnalysisError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "admission": admission, "analysis_order": analysis_order, "rank_order": rank_order, "decisions": decisions, "checkpoint_digest": checkpoint_digest, "control_digest": control_digest, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY}); let artifact = TypedResearchArtifact::from_payload(format!("federated-analysis-assurance:{}", request.request_id), "application/vnd.aurora.qualified-analysis-result+json", &payload, semantic_loss.clone(), vec![ProvenanceLink { source_id: request.federation_id.clone(), relation: "federated-analysis-assurance".into(), digest: control_digest.clone() }]).map_err(|error| FederatedAnalysisError::Artifact(error.to_string()))?; let effect_receipts = if admission == AnalysisAdmission::Qualified { vec![format!("qualify:analysis:{}", request.federation_id)] } else { vec!["block:unsafe-release".into()] };
    let receipt = FederatedAnalysisReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), federation_id: request.federation_id.clone(), purpose: request.purpose.clone(), semantic_profile: request.semantic_profile.clone(), admission, origin_order: origins.into_iter().collect(), analysis_order: candidates.iter().map(|candidate| candidate.analysis_id.clone()).collect(), rank_order, qualified_order: qualified, unresolved_order: unresolved, blocked_order: blocked, decisions, checkpoint_seq: request.checkpoint_seq, checkpoint_digest, control_digest, replay_identity: request.replay_identity.clone(), semantic_loss, omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts, artifact, raw_data_local: request.raw_data_local, boundary: PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash { ContentHash::of_bytes(b"federated-analysis") }
    fn candidate(id: &str, origin: &str, state: EvidenceState) -> AnalysisCandidate { AnalysisCandidate { analysis_id: id.into(), origin: origin.into(), scope: "preclinical".into(), semantic_profile: "analysis-v1".into(), estimand: "effect".into(), metric_digest: hash(), data_digest: hash(), provenance_digest: hash(), replay_identity: hash(), evidence_state: state, baseline_delta_milli: 5000, uncertainty_width_milli: 100, independent_site_count: 3, required_site_quorum: 2, policy_allow: true, protected_closure: true, signed_approval: true, raw_data_local: true, negative_result: false } }
    fn request() -> FederatedAnalysisRequest { FederatedAnalysisRequest { request_id: "request:analysis".into(), federation_id: "federation:analysis".into(), purpose: "analysis-release".into(), semantic_profile: "analysis-v1".into(), required_origin_quorum: 2, capacity: 4, active_runs: 1, checkpoint_seq: 1, candidates: vec![candidate("a1", "site-a", EvidenceState::Supported), candidate("a2", "site-b", EvidenceState::Proven)], policy_allow: true, protected_closure: true, signed_approval: true, network_permitted: true, raw_data_local: true, replay_identity: hash(), boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1_and_operator_facing() { assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1); assert!(capability_manifest().surfaces.contains(&ResearchSurface::Operator)); }
    #[test] fn qualified_analysis_is_deterministic() { let receipt = assure(&request()).unwrap(); assert_eq!(receipt.admission, AnalysisAdmission::Qualified); assert_eq!(receipt.rank_order, vec!["a2", "a1"]); }
    #[test] fn unknown_or_site_gap_is_unresolved() { let mut value = request(); value.candidates[0].evidence_state = EvidenceState::Unknown; value.candidates[0].independent_site_count = 1; let receipt = assure(&value).unwrap(); assert_eq!(receipt.admission, AnalysisAdmission::Unresolved); }
    #[test] fn contradiction_and_policy_block() { let mut value = request(); value.candidates[0].evidence_state = EvidenceState::Contradicted; value.policy_allow = false; let receipt = assure(&value).unwrap(); assert_eq!(receipt.admission, AnalysisAdmission::Blocked); assert!(receipt.negative_evidence.iter().any(|item| item.contains("contradicted"))); }
    #[test] fn locality_and_approval_fail_closed() { let mut value = request(); value.candidates[0].raw_data_local = false; value.signed_approval = false; let receipt = assure(&value).unwrap(); assert_eq!(receipt.admission, AnalysisAdmission::Blocked); assert!(receipt.effect_receipts.contains(&"block:unsafe-release".into())); }
}
