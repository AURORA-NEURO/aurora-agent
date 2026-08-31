//! Deterministic statistical, causal, and machine-learning qualification for P13.
//!
//! The capability evaluates declared analysis candidates and emits a typed result.  It
//! never executes arbitrary model code, moves raw data, or turns incomplete evidence into
//! a positive conclusion; callers receive an explicit unresolved/blocked disposition.
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldgen-P13-F01";
pub const CONTRACT_VERSION: &str = "worldgen-local-statistical-causal-ml/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisQuestion3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedAnalysisResult1@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.statistical-causal-ml-result+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisEvidenceState { Proven, Supported, Unknown, Unmeasured, Contradicted, Negative }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate {
    pub analysis_id: String,
    pub method: String,
    pub input_order: Vec<String>,
    pub baseline_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub evidence_state: AnalysisEvidenceState,
    pub estimate_milli: i64,
    pub uncertainty_milli: u64,
    pub counterfactual_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub policy_allowed: bool,
    pub protected_closure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisQuestion3 {
    pub schema_version: String,
    pub request_id: String,
    pub study_id: String,
    pub question: String,
    pub scope: String,
    pub candidates: Vec<AnalysisCandidate>,
    pub replay_identity: ContentHash,
    pub max_candidates: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub federated_summary_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedAnalysisResult1 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub effect_estimate_milli: Vec<i64>,
    pub uncertainty_milli: Vec<u64>,
    pub counterfactual_order: Vec<String>,
    pub decisions: Vec<Value>,
    pub replay_identity: ContentHash,
    pub analysis_digest: ContentHash,
    pub semantic_loss: Vec<Value>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub contradiction: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub artifact: Value,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub federation_export: String,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StatisticalCausalMlError {
    #[error("invalid analysis question: {0}")] Invalid(String),
    #[error("analysis artifact failed: {0}")] Artifact(String),
}

fn valid_hash(h: &ContentHash) -> bool { h.as_str().len() == 64 && h.as_str().bytes().all(|b| b.is_ascii_hexdigit()) }
fn canonical(v: &[String]) -> bool { v.windows(2).all(|p| p[0] < p[1]) }
fn partition(all: &[String], parts: &[&[String]]) -> Result<(), StatisticalCausalMlError> {
    let expected = all.iter().cloned().collect::<BTreeSet<_>>();
    let flat = parts.iter().flat_map(|p| p.iter().cloned()).collect::<Vec<_>>();
    if expected.len() != all.len() || flat.len() != expected.len() || flat.iter().cloned().collect::<BTreeSet<_>>() != expected { return Err(StatisticalCausalMlError::Invalid("analysis outcomes do not partition candidates".into())); }
    Ok(())
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str) -> Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["imaging core scientist","bioinformatician","benchmark curator"],"behavior":format!("qualify declared statistical, causal, and ML analyses at {scale} with uncertainty and counterfactual receipts"),"value":"prevents leakage, unsupported certainty, unproven effects, and unauthorized data movement from entering research conclusions","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["emit:qualified-analysis-result","block:unsafe-release"],"permissions":["evaluate:declared-analysis"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

fn validate_request(r: &AnalysisQuestion3) -> Result<(), StatisticalCausalMlError> {
    if r.schema_version != INPUT_SCHEMA || [&r.request_id, &r.study_id, &r.question, &r.scope].iter().any(|v| v.trim().is_empty()) || r.candidates.is_empty() || r.candidates.len() as u32 > r.max_candidates || !valid_hash(&r.replay_identity) || !r.policy_allow || !r.raw_data_local || !r.federated_summary_only || r.boundary != PRECLINICAL_BOUNDARY || !canonical(&r.adversarial_events) { return Err(StatisticalCausalMlError::Invalid("identity, candidate bounds, locality, policy, replay, or boundary is invalid".into())); }
    let mut ids = BTreeSet::new();
    for c in &r.candidates {
        if c.analysis_id.trim().is_empty() || !ids.insert(c.analysis_id.clone()) || c.method.trim().is_empty() || !canonical(&c.input_order) || !canonical(&c.counterfactual_order) || !canonical(&c.omissions) || !canonical(&c.uncertainty) || !valid_hash(&c.replay_identity) || c.baseline_digest.as_ref().is_some_and(|h| !valid_hash(h)) || c.provenance_digest.as_ref().is_some_and(|h| !valid_hash(h)) { return Err(StatisticalCausalMlError::Invalid("candidate identifiers, ordering, evidence, or digests are invalid".into())); }
    }
    Ok(())
}

impl QualifiedAnalysisResult1 {
    pub fn validate(&self) -> Result<(), StatisticalCausalMlError> {
        if self.schema_version != "aurora-research-contract/1.0" || self.boundary != PRECLINICAL_BOUNDARY || self.artifact.get("boundary").and_then(Value::as_str) != Some(PRECLINICAL_BOUNDARY) || self.artifact.get("content_type").and_then(Value::as_str) != Some(CONTENT_TYPE) || self.artifact.get("content_hash").and_then(Value::as_str) != Some(self.analysis_digest.as_str()) || !self.raw_data_local || self.federation_export != "aggregate-digest-only" || self.candidate_order.is_empty() || self.decisions.len() != self.candidate_order.len() || self.effect_receipts.is_empty() || !["qualified","unresolved","blocked"].contains(&self.disposition.as_str()) || !valid_hash(&self.replay_identity) || !valid_hash(&self.analysis_digest) { return Err(StatisticalCausalMlError::Invalid("analysis identity, locality, digest, or effects are incomplete".into())); }
        for v in [&self.candidate_order, &self.selected_order, &self.unresolved_order, &self.blocked_order, &self.omitted_order, &self.counterfactual_order, &self.omissions, &self.uncertainty, &self.contradiction, &self.negative_evidence, &self.effect_receipts] { if !canonical(v) { return Err(StatisticalCausalMlError::Invalid("analysis vectors are not canonical".into())); } }
        partition(&self.candidate_order, &[&self.selected_order, &self.unresolved_order, &self.blocked_order])?;
        if self.effect_estimate_milli.len() != self.selected_order.len() || self.uncertainty_milli.len() != self.selected_order.len() { return Err(StatisticalCausalMlError::Invalid("selected effects and uncertainty do not align".into())); }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, StatisticalCausalMlError> { self.validate()?; ContentHash::of_value(&serde_json::to_value(self).map_err(|e| StatisticalCausalMlError::Artifact(e.to_string()))?).map_err(|e| StatisticalCausalMlError::Artifact(e.to_string())) }
}

pub fn qualify(r: &AnalysisQuestion3, feature_id: &str, contract_version: &str) -> Result<QualifiedAnalysisResult1, StatisticalCausalMlError> {
    validate_request(r)?;
    let mut cs = r.candidates.clone(); cs.sort_by(|a,b| a.analysis_id.cmp(&b.analysis_id));
    let candidates = cs.iter().map(|c| c.analysis_id.clone()).collect::<Vec<_>>();
    let global_block = !r.protected_closure || !r.policy_allow || !r.raw_data_local || !r.federated_summary_only || !r.adversarial_events.is_empty();
    let mut selected = Vec::new(); let mut unresolved = Vec::new(); let mut blocked = Vec::new(); let mut omitted = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut contradiction = BTreeSet::new(); let mut negative = BTreeSet::new(); let mut counterfactual = BTreeSet::new(); let mut estimates = Vec::new(); let mut widths = Vec::new(); let mut decisions = Vec::new(); let mut loss = Vec::new();
    for c in &cs {
        omitted.extend(c.omissions.iter().map(|x| format!("{}:{}", c.analysis_id, x))); uncertainty.extend(c.uncertainty.iter().map(|x| format!("{}:{}", c.analysis_id, x))); counterfactual.extend(c.counterfactual_order.iter().map(|x| format!("{}:{}", c.analysis_id, x)));
        if c.evidence_state == AnalysisEvidenceState::Contradicted { contradiction.insert(format!("{}:contradicted", c.analysis_id)); }
        if c.evidence_state == AnalysisEvidenceState::Negative { negative.insert(format!("{}:null-or-negative-result", c.analysis_id)); }
        let fail = global_block || !c.policy_allowed || !c.protected_closure || c.baseline_digest.is_none() || c.provenance_digest.is_none() || c.replay_identity != r.replay_identity;
        let pending = !fail && (!matches!(c.evidence_state, AnalysisEvidenceState::Proven | AnalysisEvidenceState::Supported) || !c.omissions.is_empty() || !c.uncertainty.is_empty() || c.uncertainty_milli == 0 || c.counterfactual_order.is_empty());
        let state = if fail { "blocked" } else if pending { "unresolved" } else { "selected" };
        match state { "blocked" => { blocked.push(c.analysis_id.clone()); loss.push(json!({"field":format!("candidate:{}",c.analysis_id),"reason":"analysis gate failed","severity":"decision_relevant"})); }, "unresolved" => unresolved.push(c.analysis_id.clone()), _ => { selected.push(c.analysis_id.clone()); estimates.push(c.estimate_milli); widths.push(c.uncertainty_milli); } }
        decisions.push(json!({"analysis_id":c.analysis_id,"method":c.method,"disposition":state,"estimate_milli":c.estimate_milli,"uncertainty_milli":c.uncertainty_milli}));
    }
    if global_block { selected.clear(); unresolved.clear(); blocked = candidates.clone(); omitted.insert("request:policy-or-locality-blocked".into()); }
    selected.sort(); unresolved.sort(); blocked.sort(); let disposition = if global_block || !blocked.is_empty() { "blocked" } else if !unresolved.is_empty() { "unresolved" } else { "qualified" }; if disposition != "qualified" { omitted.insert("request:analysis-closure-not-ready".into()); }
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":contract_version,"feature_id":feature_id,"request_id":r.request_id,"study_id":r.study_id,"scope":r.scope,"disposition":disposition,"candidate_order":candidates,"selected_order":selected,"unresolved_order":unresolved,"blocked_order":blocked,"omitted_order":omitted,"effect_estimate_milli":estimates,"uncertainty_milli":widths,"counterfactual_order":counterfactual,"decisions":decisions,"replay_identity":r.replay_identity,"semantic_loss":loss,"omissions":omitted,"uncertainty":uncertainty,"contradiction":contradiction,"negative_evidence":negative,"raw_data_local":true,"federation_export":"aggregate-digest-only","boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload).map_err(|e| StatisticalCausalMlError::Artifact(e.to_string()))?;
    let mut full = payload; full["analysis_digest"] = json!(digest); full["artifact"] = json!({"artifact_id":format!("qualified-analysis-result-1:{}",r.request_id),"content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":full["semantic_loss"],"provenance":[{"source_id":r.study_id,"relation":"statistical-causal-ml-qualification","digest":digest}],"boundary":PRECLINICAL_BOUNDARY}); full["effect_receipts"] = json!(if disposition == "qualified" { vec![format!("qualify:analysis:{}",r.request_id)] } else { vec!["block:unsafe-release".to_string()] });
    let out: QualifiedAnalysisResult1 = serde_json::from_value(full).map_err(|e| StatisticalCausalMlError::Artifact(e.to_string()))?; out.validate()?; Ok(out)
}
