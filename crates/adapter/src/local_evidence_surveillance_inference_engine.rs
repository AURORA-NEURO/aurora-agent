//! Local single-study evidence-surveillance inference engine.
//!
//! Atlas feature: `AFA-adapter-P01-F01`. This is a distinct A0 scientific algorithm from the
//! adapter's copilot: it computes a reproducible qualified evidence stream and preserves every
//! source that failed scope, availability, digest, relevance, or evidence-state gates.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F01";
pub const CONTRACT_VERSION: &str = "adapter-local-evidence-surveillance-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceObservation {
    pub source_id: String,
    pub study_id: String,
    pub source_type: String,
    pub locator: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceRequest {
    pub request_id: String,
    pub study_id: String,
    pub intent: String,
    pub required_source_ids: Vec<String>,
    pub observations: Vec<LocalEvidenceObservation>,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalEvidenceSurveillanceDisposition { Completed, Partial, Unknown, Blocked }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub study_id: String,
    pub intent: String,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub negative_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub evidence_state: EvidenceState,
    pub ordering_rule: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub intent: String,
    pub disposition: LocalEvidenceSurveillanceDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: LocalQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocalEvidenceSurveillanceError {
    #[error("invalid local evidence surveillance request: {0}")]
    Invalid(String),
    #[error("local evidence surveillance artifact failed: {0}")]
    Artifact(String),
}

impl LocalEvidenceSurveillanceReceipt {
    pub fn validate(&self) -> Result<(), LocalEvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.study_id != self.study_id
            || self.qualified_set.intent != self.intent
        { return Err(LocalEvidenceSurveillanceError::Invalid("local surveillance identity, locality, candidates, effects, or qualified-set linkage is incomplete".into())); }
        for values in [&self.candidate_order, &self.selected_order, &self.unresolved_order, &self.denied_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts, &self.qualified_set.selected_order, &self.qualified_set.negative_order, &self.qualified_set.omissions, &self.qualified_set.uncertainty] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) { return Err(LocalEvidenceSurveillanceError::Invalid("local surveillance ordering is not canonical".into())); }
        }
        let classified = self.selected_order.iter().chain(self.unresolved_order.iter()).chain(self.denied_order.iter()).cloned().collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len() || classified.iter().any(|source| !self.candidate_order.contains(source)) || self.qualified_set.selected_order != self.selected_order { return Err(LocalEvidenceSurveillanceError::Invalid("local surveillance states do not partition candidates".into())); }
        for digest in [&self.replay_identity, &self.evidence_digest, &self.provenance_digest, &self.artifact.content_hash] { if digest.as_str().len() != 64 { return Err(LocalEvidenceSurveillanceError::Invalid("local surveillance digest is invalid".into())); } }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("read:local-evidence-surveillance:") && effect != "block:unsafe-release") { return Err(LocalEvidenceSurveillanceError::Invalid("local surveillance effect is outside read-only gate".into())); }
        self.artifact.validate_metadata().map_err(|error| LocalEvidenceSurveillanceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, LocalEvidenceSurveillanceError> { self.validate()?; let value = serde_json::to_value(self).map_err(|error| LocalEvidenceSurveillanceError::Artifact(error.to_string()))?; ContentHash::of_value(&value).map_err(|error| LocalEvidenceSurveillanceError::Artifact(error.to_string())) }
}

pub fn local_evidence_surveillance_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["consortium administrator".into(), "evidence steward".into()].into(), behavior: "deterministically computes an institution-local qualified evidence stream while retaining omissions, negative results, and uncertainty".into(), value: "raises auditable evidence discovery without silently promoting unavailable, unmeasured, or contradicted sources".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_local_evidence_surveillance(request: &LocalEvidenceSurveillanceRequest) -> Result<LocalEvidenceSurveillanceReceipt, LocalEvidenceSurveillanceError> {
    if request.request_id.trim().is_empty() || request.study_id.trim().is_empty() || request.intent.trim().is_empty() || request.observations.is_empty() || request.replay_identity.as_str().len() != 64 || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local { return Err(LocalEvidenceSurveillanceError::Invalid("local surveillance identity, observations, replay, locality, or boundary is invalid".into())); }
    let mut observations = request.observations.clone(); observations.sort_by(|left, right| right.relevance_score.cmp(&left.relevance_score).then_with(|| left.source_id.cmp(&right.source_id)));
    let candidate = observations.iter().map(|observation| observation.source_id.clone()).collect::<Vec<_>>(); if candidate.windows(2).any(|pair| pair[0] == pair[1]) || candidate.iter().any(|value| value.trim().is_empty()) { return Err(LocalEvidenceSurveillanceError::Invalid("observation source identities must be unique and non-empty".into())); }
    let mut selected = BTreeSet::new(); let mut selected_digest_map = BTreeMap::new(); let mut unresolved = BTreeSet::new(); let mut denied = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    for observation in &observations {
        if observation.study_id != request.study_id || observation.locator.trim().is_empty() || observation.source_type.trim().is_empty() || !request.policy_allow || !request.protected_closure { denied.insert(observation.source_id.clone()); omissions.insert(format!("source:{}:scope-policy-closure", observation.source_id)); }
        else if observation.availability != EvidenceAvailability::Available { unresolved.insert(observation.source_id.clone()); omissions.insert(format!("source:{}:availability-{:?}", observation.source_id, observation.availability)); }
        else if observation.relevance_score < request.min_relevance_score { unresolved.insert(observation.source_id.clone()); uncertainty.insert(format!("source:{}:relevance-below-threshold", observation.source_id)); }
        else if observation.digest.is_none() { unresolved.insert(observation.source_id.clone()); omissions.insert(format!("source:{}:content-digest-missing", observation.source_id)); }
        else if matches!(observation.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative) { unresolved.insert(observation.source_id.clone()); uncertainty.insert(format!("source:{}:unknown-not-asserted", observation.source_id)); }
        else if matches!(observation.evidence_state, EvidenceState::Contradicted) { denied.insert(observation.source_id.clone()); negative.insert(format!("source:{}:contradicted", observation.source_id)); }
        else { selected.insert(observation.source_id.clone()); selected_digest_map.insert(observation.source_id.clone(), observation.digest.clone().expect("digest checked")); if observation.negative_result { negative.insert(format!("source:{}:negative-result", observation.source_id)); } }
    }
    for required in request.required_source_ids.iter().collect::<BTreeSet<_>>() { if !selected.contains(required) { omissions.insert(format!("source:{}:required-not-qualified", required)); uncertainty.insert(format!("source:{}:required-unresolved", required)); } }
    if !request.policy_allow { omissions.insert("control:policy-denied".into()); } if !request.protected_closure { omissions.insert("control:protected-closure-incomplete".into()); }
    let selected_order = selected.iter().cloned().collect::<Vec<_>>(); let selected_digests = selected_order.iter().filter_map(|source| selected_digest_map.get(source).cloned()).collect::<Vec<_>>(); let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { LocalEvidenceSurveillanceDisposition::Blocked } else if selected.is_empty() { LocalEvidenceSurveillanceDisposition::Unknown } else if !unresolved.is_empty() || !denied.is_empty() || request.required_source_ids.iter().any(|required| !selected.contains(required)) { LocalEvidenceSurveillanceDisposition::Partial } else { LocalEvidenceSurveillanceDisposition::Completed };
    let evidence_digest = ContentHash::of_value(&json!({"candidate_order": candidate.clone(), "selected_order": selected_order.clone(), "unresolved_order": unresolved.clone(), "denied_order": denied.clone()})).map_err(|error| LocalEvidenceSurveillanceError::Artifact(error.to_string()))?; let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "study_id": request.study_id, "replay_identity": request.replay_identity, "evidence_digest": evidence_digest})).map_err(|error| LocalEvidenceSurveillanceError::Artifact(error.to_string()))?; let state = if disposition == LocalEvidenceSurveillanceDisposition::Completed { EvidenceState::Supported } else { EvidenceState::Unknown }; let qualified_set = LocalQualifiedEvidenceSet { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), set_id: format!("qualified-evidence-local:{}", request.request_id), study_id: request.study_id.clone(), intent: request.intent.clone(), selected_order: selected_order.clone(), selected_digests, negative_order: negative.iter().cloned().collect(), omissions: omissions.iter().cloned().collect(), uncertainty: uncertainty.iter().cloned().collect(), evidence_state: state, ordering_rule: "relevance_score descending, source_id ascending; artifact digests ascending".into(), boundary: PRECLINICAL_BOUNDARY.into() }; let payload = serde_json::to_value(&qualified_set).map_err(|error| LocalEvidenceSurveillanceError::Artifact(error.to_string()))?; let artifact = TypedResearchArtifact::from_payload(qualified_set.set_id.clone(), "application/vnd.aurora.qualified-evidence-set+json", &payload, Vec::new(), Vec::new()).map_err(|error| LocalEvidenceSurveillanceError::Artifact(error.to_string()))?; let receipt = LocalEvidenceSurveillanceReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), study_id: request.study_id.clone(), intent: request.intent.clone(), disposition, candidate_order: candidate, selected_order, unresolved_order: unresolved.into_iter().collect(), denied_order: denied.into_iter().collect(), replay_identity: request.replay_identity.clone(), evidence_digest, provenance_digest, omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: if disposition == LocalEvidenceSurveillanceDisposition::Blocked { vec!["block:unsafe-release".into()] } else { vec![format!("read:local-evidence-surveillance:{}", request.request_id)] }, qualified_set, artifact, raw_data_local: request.raw_data_local, boundary: PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

#[cfg(test)] mod tests { use super::*; fn hash(v:&str)->ContentHash{ContentHash::of_bytes(v.as_bytes())} fn request()->LocalEvidenceSurveillanceRequest{let h=hash("local-evidence");let observation=|id:&str,score:u16,state:EvidenceState|LocalEvidenceObservation{source_id:id.into(),study_id:"study:one".into(),source_type:"paper".into(),locator:format!("local://{id}"),digest:Some(h.clone()),availability:EvidenceAvailability::Available,evidence_state:state,relevance_score:score,negative_result:id=="source:b"};LocalEvidenceSurveillanceRequest{request_id:"request:local-evidence".into(),study_id:"study:one".into(),intent:"monitor mechanism".into(),required_source_ids:vec!["source:a".into()],observations:vec![observation("source:a",90,EvidenceState::Supported),observation("source:b",80,EvidenceState::Supported)],min_relevance_score:70,policy_allow:true,protected_closure:true,raw_data_local:true,replay_identity:h,boundary:PRECLINICAL_BOUNDARY.into()}} #[test] fn manifest_is_a0(){assert_eq!(local_evidence_surveillance_inference_engine_manifest().autonomy_tier,AutonomyTier::A0);} #[test] fn supported_feed_completes(){let r=run_local_evidence_surveillance(&request()).unwrap();assert_eq!(r.disposition,LocalEvidenceSurveillanceDisposition::Completed);assert!(r.negative_evidence.iter().any(|x|x.contains("negative-result")));} #[test] fn stale_source_is_partial(){let mut v=request();v.observations[0].availability=EvidenceAvailability::Stale;assert_eq!(run_local_evidence_surveillance(&v).unwrap().disposition,LocalEvidenceSurveillanceDisposition::Partial);} #[test] fn missing_digest_is_partial(){let mut v=request();v.observations[0].digest=None;assert_eq!(run_local_evidence_surveillance(&v).unwrap().disposition,LocalEvidenceSurveillanceDisposition::Partial);} #[test] fn unknown_is_not_asserted(){let mut v=request();v.observations[0].evidence_state=EvidenceState::Unknown;let r=run_local_evidence_surveillance(&v).unwrap();assert!(r.uncertainty.iter().any(|x|x.contains("unknown-not-asserted")));} #[test] fn contradiction_is_denied(){let mut v=request();v.observations[0].evidence_state=EvidenceState::Contradicted;assert!(run_local_evidence_surveillance(&v).unwrap().denied_order.contains(&"source:a".to_string()));} #[test] fn policy_blocks(){let mut v=request();v.policy_allow=false;let r=run_local_evidence_surveillance(&v).unwrap();assert_eq!(r.disposition,LocalEvidenceSurveillanceDisposition::Blocked);assert_eq!(r.effect_receipts,vec!["block:unsafe-release"]);} #[test] fn digest_is_stable(){let r=run_local_evidence_surveillance(&request()).unwrap();assert_eq!(r.digest().unwrap(),r.digest().unwrap());} }
