//! Prospective high-throughput statistical/causal/ML analysis assurance harness
//! (`AFA-bioethics-P13-F27`).
//!
//! The harness verifies typed analysis-model declarations before execution. It checks estimand,
//! evidence, quality, replay, provenance, ethical review, policy, protected closure, and local
//! data gates. It never fits a model, reads raw arrays, makes a causal claim, or makes a clinical
//! decision.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioethics-P13-F27";
pub const CONTRACT_VERSION: &str = "bioethics-prospective-statistical-causal-ml-analysis-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisQuestion3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedAnalysisResult7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.bioethics-qualified-analysis-result-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate4 {
    pub model_id: String,
    pub estimand: String,
    pub scope: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub quality_milli: u16,
    pub permitted: bool,
    pub local_only: bool,
    pub privacy_reviewed: bool,
    pub dual_use_reviewed: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisQuestion3 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub required_estimand: String,
    pub minimum_quality_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub institutional_authorized: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub boundary: String,
    pub candidates: Vec<AnalysisCandidate4>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedAnalysisArtifact7 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedAnalysisResult7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub required_estimand: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub analysis_digest: ContentHash,
    pub artifact: QualifiedAnalysisArtifact7,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StatisticalAnalysisAssuranceError {
    #[error("invalid statistical analysis assurance request or receipt: {0}")]
    Invalid(String),
    #[error("statistical analysis assurance artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool { values.windows(2).all(|w| w[0] < w[1]) }
fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit()) }

pub fn statistical_analysis_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioethics".into(), consumers: ["institutional safety reviewer".into(), "analysis portfolio steward".into(), "prospective workflow operator".into()].into(), behavior: "verify prospective high-throughput statistical, causal, and ML analysis declarations with ethical, evidence, provenance, replay, quality, and policy gates".into(), value: "prevents unsupported or ethically unreviewed analytical results from being mistaken for qualified research findings".into(), inputs: vec![TypedPort { name: "analysis_question".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_analysis_result".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

impl QualifiedAnalysisResult7 {
    pub fn validate(&self) -> Result<(), StatisticalAnalysisAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.boundary != PRECLINICAL_BOUNDARY || self.artifact.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local || !self.aggregate_only || !matches!(self.disposition.as_str(), "qualified" | "partial" | "blocked") || self.candidate_order.is_empty() || self.effect_receipts.is_empty() || [&self.request_id, &self.consumer, &self.purpose, &self.target_scope, &self.semantic_profile, &self.required_estimand].iter().any(|v| v.trim().is_empty()) { return Err(StatisticalAnalysisAssuranceError::Invalid("analysis identity, locality, candidates, or effects are incomplete".into())); }
        for values in [&self.candidate_order, &self.qualified_order, &self.unresolved_order, &self.blocked_order, &self.omission_order, &self.uncertainty_order, &self.negative_evidence_order, &self.effect_receipts] { if !ordered(values) { return Err(StatisticalAnalysisAssuranceError::Invalid("analysis ordering is not canonical".into())); } }
        let ids = self.candidate_order.iter().cloned().collect::<BTreeSet<_>>(); let states = self.qualified_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).cloned().collect::<Vec<_>>(); if ids.len() != self.candidate_order.len() || states.len() != ids.len() || states.iter().cloned().collect::<BTreeSet<_>>() != ids { return Err(StatisticalAnalysisAssuranceError::Invalid("analysis candidate states do not partition".into())); }
        if !digest(&self.replay_identity) || !digest(&self.analysis_digest) || self.artifact.content_hash != self.analysis_digest || self.artifact.content_type != CONTENT_TYPE || !self.artifact.provenance_digests.iter().all(digest) { return Err(StatisticalAnalysisAssuranceError::Artifact("analysis digest is inconsistent".into())); }
        if self.effect_receipts.iter().any(|e| e != "block:unsafe-release" && !e.starts_with("observe:analysis:")) { return Err(StatisticalAnalysisAssuranceError::Invalid("analysis effect is outside assurance gate".into())); }
        if self.disposition == "qualified" && self.effect_receipts != [format!("observe:analysis:{}", self.request_id)] { return Err(StatisticalAnalysisAssuranceError::Invalid("qualified analysis effect is invalid".into())); }
        if self.disposition != "qualified" && self.effect_receipts != ["block:unsafe-release"] { return Err(StatisticalAnalysisAssuranceError::Invalid("non-qualified analysis must block".into())); }
        Ok(())
    }
}

pub fn assure_statistical_analysis(request: &AnalysisQuestion3) -> Result<QualifiedAnalysisResult7, StatisticalAnalysisAssuranceError> {
    if request.schema_version != INPUT_SCHEMA || request.request_id.trim().is_empty() || request.consumer.trim().is_empty() || request.purpose.trim().is_empty() || request.target_scope.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.required_estimand.trim().is_empty() || request.minimum_quality_milli == 0 || request.candidates.is_empty() || !digest(&request.replay_identity) || !request.aggregate_only || !request.raw_data_local || request.boundary != PRECLINICAL_BOUNDARY { return Err(StatisticalAnalysisAssuranceError::Invalid("analysis query identity, bounds, replay, locality, or boundary is invalid".into())); }
    let candidate_order = request.candidates.iter().map(|c| c.model_id.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(); if candidate_order.len() != request.candidates.len() || candidate_order.iter().any(|id| id.trim().is_empty()) { return Err(StatisticalAnalysisAssuranceError::Invalid("model ids must be unique and non-empty".into())); }
    let mut qualified = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.negative_result { negative.insert(candidate.model_id.clone()); }
        if !candidate.omission_order.is_empty() { omissions.extend(candidate.omission_order.iter().map(|o| format!("{}:{}", candidate.model_id, o))); }
        let hard = !candidate.permitted || !candidate.local_only || !candidate.privacy_reviewed || !candidate.dual_use_reviewed || candidate.scope != request.target_scope || candidate.semantic_profile != request.semantic_profile || candidate.estimand != request.required_estimand || candidate.quality_milli < request.minimum_quality_milli || !digest(&candidate.artifact_digest) || !digest(&candidate.provenance_digest) || candidate.replay_identity != request.replay_identity;
        if hard { blocked.insert(candidate.model_id.clone()); omissions.insert(format!("{}:analysis-integrity-or-ethics", candidate.model_id)); }
        else if matches!(candidate.evidence_state, EvidenceState::Contradicted | EvidenceState::Unknown) { unresolved.insert(candidate.model_id.clone()); uncertainty.insert(format!("{}:evidence-state", candidate.model_id)); }
        else { qualified.insert(candidate.model_id.clone()); }
    }
    for (flag, label) in [(request.policy_allow, "workflow:policy-denied"), (request.protected_closure, "workflow:protected-closure-incomplete"), (request.institutional_authorized, "workflow:institutional-authorization-missing")] { if !flag { omissions.insert(label.into()); } }
    let global_block = !request.policy_allow || !request.protected_closure || !request.institutional_authorized; let disposition = if global_block || !blocked.is_empty() { "blocked" } else if !unresolved.is_empty() { "partial" } else { "qualified" }; if global_block { blocked.extend(candidate_order.iter().cloned()); qualified.clear(); unresolved.clear(); }
    let checkpoint = ContentHash::of_value(&json!({"request_id":request.request_id,"target_scope":request.target_scope,"semantic_profile":request.semantic_profile,"required_estimand":request.required_estimand,"replay_identity":request.replay_identity})).map_err(|e| StatisticalAnalysisAssuranceError::Artifact(e.to_string()))?; let payload = json!({"candidate_order":candidate_order,"qualified_order":qualified,"unresolved_order":unresolved,"blocked_order":blocked,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"checkpoint":checkpoint,"replay_identity":request.replay_identity}); let analysis_digest = ContentHash::of_value(&payload).map_err(|e| StatisticalAnalysisAssuranceError::Artifact(e.to_string()))?; let strings = |k:&str| payload[k].as_array().map(|a|a.iter().filter_map(|v|v.as_str().map(String::from)).collect()).unwrap_or_default(); let receipt = QualifiedAnalysisResult7 { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:CONTRACT_VERSION.into(), feature_id:FEATURE_ID.into(), request_id:request.request_id.clone(), consumer:request.consumer.clone(), purpose:request.purpose.clone(), target_scope:request.target_scope.clone(), semantic_profile:request.semantic_profile.clone(), required_estimand:request.required_estimand.clone(), disposition:disposition.into(), candidate_order:strings("candidate_order"), qualified_order:strings("qualified_order"), unresolved_order:strings("unresolved_order"), blocked_order:strings("blocked_order"), omission_order:strings("omission_order"), uncertainty_order:strings("uncertainty_order"), negative_evidence_order:strings("negative_evidence_order"), replay_identity:request.replay_identity.clone(), analysis_digest:analysis_digest.clone(), artifact:QualifiedAnalysisArtifact7{artifact_id:format!("bioethics-analysis:{}",request.request_id),content_type:CONTENT_TYPE.into(),content_hash:analysis_digest,semantic_loss:if disposition=="qualified"{Vec::new()}else{vec!["analysis-not-executed".into()]},provenance_digests:request.candidates.iter().map(|c|c.provenance_digest.clone()).collect(),boundary:PRECLINICAL_BOUNDARY.into()},effect_receipts:if disposition=="qualified"{vec![format!("observe:analysis:{}",request.request_id)]}else{vec!["block:unsafe-release".into()]},raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()}; receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests { use super::*; fn h(v:&str)->ContentHash{ContentHash::of_bytes(v.as_bytes())} fn request()->AnalysisQuestion3{AnalysisQuestion3{schema_version:INPUT_SCHEMA.into(),request_id:"analysis-1".into(),consumer:"reviewer".into(),purpose:"prospective analysis assurance".into(),target_scope:"organoid".into(),semantic_profile:"analysis:v1".into(),required_estimand:"effect-on-growth".into(),minimum_quality_milli:800,replay_identity:h("replay"),policy_allow:true,protected_closure:true,institutional_authorized:true,aggregate_only:true,raw_data_local:true,boundary:PRECLINICAL_BOUNDARY.into(),candidates:vec![AnalysisCandidate4{model_id:"m1".into(),estimand:"effect-on-growth".into(),scope:"organoid".into(),semantic_profile:"analysis:v1".into(),artifact_digest:h("artifact"),provenance_digest:h("prov"),replay_identity:h("replay"),evidence_state:EvidenceState::Supported,quality_milli:900,permitted:true,local_only:true,privacy_reviewed:true,dual_use_reviewed:true,negative_result:false,omission_order:vec![]}]}} #[test]fn manifest_is_a1(){assert_eq!(statistical_analysis_assurance_manifest().autonomy_tier,AutonomyTier::A1)} #[test]fn qualified_analysis(){assert_eq!(assure_statistical_analysis(&request()).unwrap().disposition,"qualified")} #[test]fn low_quality_blocks(){let mut r=request();r.candidates[0].quality_milli=100;assert_eq!(assure_statistical_analysis(&r).unwrap().disposition,"blocked")} #[test]fn policy_blocks(){let mut r=request();r.policy_allow=false;assert_eq!(assure_statistical_analysis(&r).unwrap().disposition,"blocked")} }
