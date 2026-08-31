//! Prospective high-throughput interpretation and visualisation inference engine.
//!
//! Atlas feature: `AFA-oraclex-P14-F03`.  The engine ranks caller-supplied interpretation panels
//! and emits a content-addressed, read-only receipt.  It never renders a figure, infers a clinical
//! conclusion, or moves raw preclinical data.
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-oraclex-P14-F03";
pub const CONTRACT_VERSION: &str = "oraclex-prospective-interpretation-inference/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult3@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation1@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.interactive-interpretation-1+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState { Proven, Supported, Unknown, Unmeasured, Contradicted, Negative }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceBackedResult3 {
    pub schema_version: String, pub request_id: String, pub consumer: String, pub purpose: String,
    pub semantic_profile: String, pub required_study_order: Vec<String>, pub required_modality_order: Vec<String>,
    pub comparability_digest: ContentHash, pub replay_identity: ContentHash,
    pub candidates: Vec<InterpretationCandidate3>, pub policy_allow: bool, pub protected_closure: bool,
    pub federation_allow: bool, pub raw_data_local: bool, pub aggregate_only: bool, pub budget_units: u32,
    pub adversarial_event_order: Vec<String>, pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationCandidate3 {
    pub candidate_id: String, pub study_order: Vec<String>, pub modality_order: Vec<String>,
    pub semantic_profile: String, pub score_milli: u16, pub evidence_state: EvidenceState,
    pub comparability_digest: ContentHash, pub result_digest: ContentHash, pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash, pub local: bool, pub aggregate_only: bool, pub policy_allowed: bool,
    pub omission_order: Vec<String>, pub uncertainty_order: Vec<String>, pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractiveInterpretation1 {
    pub schema_version: String, pub contract_version: String, pub feature_id: String, pub request_id: String,
    pub consumer: String, pub purpose: String, pub semantic_profile: String, pub disposition: String,
    pub candidate_order: Vec<String>, pub qualified_order: Vec<String>, pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>, pub incomparable_order: Vec<String>, pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>, pub omission_order: Vec<String>, pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>, pub replay_identity: ContentHash, pub interpretation_digest: ContentHash,
    pub artifact: Value, pub effect_receipts: Vec<String>, pub raw_data_local: bool, pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error)]
pub enum InterpretationInferenceError { #[error("invalid interpretation request: {0}")] Invalid(String), #[error("invalid interpretation output: {0}")] Output(String), #[error("serialization error: {0}")] Serialization(String) }

fn hash_ok(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }
fn ordered(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn digest(value: &Value) -> Result<ContentHash, InterpretationInferenceError> { ContentHash::of_value(value).map_err(|error| InterpretationInferenceError::Serialization(error.to_string())) }

pub fn interpretation_inference_manifest() -> Value { json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"oraclex","consumers":["bioinformatician","interpretation reviewer"],"behavior":"rank typed multimodal interpretation candidates and retain evidence, comparability, and release gates","value":"turns high-throughput interpretation panels into replayable, auditable, read-only research artifacts","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"permissions":["read:local-research-artifacts"],"autonomy_tier":"A1","standards":["w3c-prov-o","ro-crate-1.3","ga4gh-drs-1.3","ome-ngff-rfc5"],"boundary":PRECLINICAL_BOUNDARY}) }

fn validate_request(request: &EvidenceBackedResult3) -> Result<(), InterpretationInferenceError> {
    if request.schema_version != INPUT_SCHEMA || request.request_id.trim().is_empty() || request.consumer.trim().is_empty() || request.purpose.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.required_study_order.is_empty() || request.required_modality_order.is_empty() || !ordered(&request.required_study_order) || !ordered(&request.required_modality_order) || !hash_ok(&request.comparability_digest) || !hash_ok(&request.replay_identity) || request.candidates.is_empty() || request.budget_units == 0 || !request.raw_data_local || !request.aggregate_only || request.boundary != PRECLINICAL_BOUNDARY { return Err(InterpretationInferenceError::Invalid("identity, closure, digest, budget, locality, or boundary is invalid".into())); }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates { if candidate.candidate_id.trim().is_empty() || !ids.insert(candidate.candidate_id.clone()) || candidate.study_order.is_empty() || candidate.modality_order.is_empty() || !ordered(&candidate.study_order) || !ordered(&candidate.modality_order) || candidate.score_milli > 1000 || !hash_ok(&candidate.comparability_digest) || !hash_ok(&candidate.result_digest) || !hash_ok(&candidate.provenance_digest) || !hash_ok(&candidate.replay_identity) || !ordered(&candidate.omission_order) || !ordered(&candidate.uncertainty_order) { return Err(InterpretationInferenceError::Invalid("candidate identity, axes, score, digest, or order is invalid".into())); } }
    Ok(())
}

impl InteractiveInterpretation1 { pub fn validate(&self) -> Result<(), InterpretationInferenceError> { if self.schema_version != "aurora-research-contract/1.0" || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.request_id.trim().is_empty() || self.consumer.trim().is_empty() || self.purpose.trim().is_empty() || self.semantic_profile.trim().is_empty() || self.candidate_order.is_empty() || self.effect_receipts != vec!["block:unsafe-release"] || !self.raw_data_local || !self.aggregate_only || self.boundary != PRECLINICAL_BOUNDARY { return Err(InterpretationInferenceError::Output("identity, locality, evidence, or release gate is incomplete".into())); } for values in [&self.candidate_order,&self.qualified_order,&self.unresolved_order,&self.blocked_order,&self.incomparable_order,&self.missing_study_order,&self.missing_modality_order,&self.omission_order,&self.uncertainty_order,&self.negative_evidence_order,&self.effect_receipts] { if !ordered(values) { return Err(InterpretationInferenceError::Output("interpretation ordering is not canonical".into())); } } let ids=self.candidate_order.iter().cloned().collect::<BTreeSet<_>>(); let parts=self.qualified_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).chain(&self.incomparable_order).cloned().collect::<Vec<_>>(); if ids.len()!=self.candidate_order.len() || parts.len()!=ids.len() || BTreeSet::from_iter(parts)!=ids { return Err(InterpretationInferenceError::Output("candidate outcomes do not partition candidates".into())); } if !hash_ok(&self.replay_identity) || !hash_ok(&self.interpretation_digest) || self.artifact.get("content_hash").and_then(Value::as_str) != Some(self.interpretation_digest.as_str()) { return Err(InterpretationInferenceError::Output("interpretation digest is invalid".into())); } Ok(()) } }

pub fn assure_interpretation(request: &EvidenceBackedResult3) -> Result<InteractiveInterpretation1, InterpretationInferenceError> {
    validate_request(request)?; let mut rows=request.candidates.clone(); rows.sort_by(|a,b| b.score_milli.cmp(&a.score_milli).then(a.candidate_id.cmp(&b.candidate_id))); let ids=rows.iter().map(|c|c.candidate_id.clone()).collect::<Vec<_>>(); let mut q=BTreeSet::new(); let mut u=BTreeSet::new(); let mut b=BTreeSet::new(); let mut i=BTreeSet::new(); let mut ms=BTreeSet::new(); let mut mm=BTreeSet::new(); let mut om=BTreeSet::new(); let mut un=BTreeSet::new(); let mut neg=BTreeSet::new(); let mut prov=BTreeSet::new();
    for c in &rows { let id=c.candidate_id.clone(); prov.insert(c.provenance_digest.clone()); om.extend(c.omission_order.iter().map(|x|format!("{id}:{x}"))); un.extend(c.uncertainty_order.iter().map(|x|format!("{id}:{x}"))); if c.negative_result || c.evidence_state==EvidenceState::Negative { neg.insert(format!("{id}:negative-result")); } if !c.local || !c.aggregate_only || !c.policy_allowed || c.replay_identity!=request.replay_identity { b.insert(id); } else if request.required_study_order.iter().any(|x|!c.study_order.contains(x)) { ms.extend(request.required_study_order.iter().filter(|x|!c.study_order.contains(x)).map(|x|format!("{}:{x}",c.candidate_id))); i.insert(c.candidate_id.clone()); } else if request.required_modality_order.iter().any(|x|!c.modality_order.contains(x)) { mm.extend(request.required_modality_order.iter().filter(|x|!c.modality_order.contains(x)).map(|x|format!("{}:{x}",c.candidate_id))); i.insert(c.candidate_id.clone()); } else if c.comparability_digest!=request.comparability_digest || c.semantic_profile!=request.semantic_profile { i.insert(c.candidate_id.clone()); un.insert(format!("{}:comparability-mismatch",c.candidate_id)); } else if !matches!(c.evidence_state,EvidenceState::Proven|EvidenceState::Supported) || c.score_milli<600 { u.insert(c.candidate_id.clone()); } else { q.insert(c.candidate_id.clone()); } }
    let global=!request.policy_allow||!request.protected_closure||!request.federation_allow||!request.raw_data_local||!request.aggregate_only||!request.adversarial_event_order.is_empty(); if global { b.extend(ids.iter().cloned()); q.clear(); u.clear(); i.clear(); om.insert("request:governance-or-adversarial-blocked".into()); } un.extend(request.adversarial_event_order.iter().map(|event|format!("adversarial:{event}"))); let qo=q.iter().cloned().collect::<Vec<_>>(); let uo=u.iter().cloned().collect::<Vec<_>>(); let bo=b.iter().cloned().collect::<Vec<_>>(); let io=i.iter().cloned().collect::<Vec<_>>(); let disposition=if global{"blocked"}else if !uo.is_empty()||!bo.is_empty()||!io.is_empty(){"unresolved"}else{"qualified"}; if disposition!="qualified"{om.insert("request:interpretation-closure-not-ready".into());}
    let mut payload=json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"consumer":request.consumer,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"candidate_order":ids,"qualified_order":qo,"unresolved_order":uo,"blocked_order":bo,"incomparable_order":io,"missing_study_order":ms.iter().cloned().collect::<Vec<_>>(),"missing_modality_order":mm.iter().cloned().collect::<Vec<_>>(),"omission_order":om.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":un.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":neg.iter().cloned().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY}); let d=digest(&payload)?; payload["interpretation_digest"]=json!(d); payload["artifact"]=json!({"artifact_id":format!("interactive-interpretation-1:{}",request.request_id),"content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omission_order"],"provenance_digests":prov.iter().cloned().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY}); payload["effect_receipts"]=json!(["block:unsafe-release"]); let out:InteractiveInterpretation1=serde_json::from_value(payload).map_err(|error|InterpretationInferenceError::Serialization(error.to_string()))?; out.validate()?; Ok(out)
}

#[cfg(test)] mod tests { use super::*; fn h(v:&str)->ContentHash{ContentHash::of_bytes(v.as_bytes())} fn fixture()->EvidenceBackedResult3{EvidenceBackedResult3{schema_version:INPUT_SCHEMA.into(),request_id:"q".into(),consumer:"bioinformatician".into(),purpose:"panel".into(),semantic_profile:"ome".into(),required_study_order:vec!["s".into()],required_modality_order:vec!["imaging".into()],comparability_digest:h("cmp"),replay_identity:h("rep"),candidates:vec![InterpretationCandidate3{candidate_id:"c".into(),study_order:vec!["s".into()],modality_order:vec!["imaging".into()],semantic_profile:"ome".into(),score_milli:800,evidence_state:EvidenceState::Supported,comparability_digest:h("cmp"),result_digest:h("res"),provenance_digest:h("prov"),replay_identity:h("rep"),local:true,aggregate_only:true,policy_allowed:true,omission_order:vec![],uncertainty_order:vec![],negative_result:false}],policy_allow:true,protected_closure:true,federation_allow:true,raw_data_local:true,aggregate_only:true,budget_units:10,adversarial_event_order:vec![],boundary:PRECLINICAL_BOUNDARY.into()}} #[test] fn release_is_explicit(){assert_eq!(assure_interpretation(&fixture()).unwrap().effect_receipts,vec!["block:unsafe-release"])} #[test] fn adversarial_blocks(){let mut r=fixture();r.adversarial_event_order=vec!["poisoned-artifact".into()];assert_eq!(assure_interpretation(&r).unwrap().disposition,"blocked")} }
