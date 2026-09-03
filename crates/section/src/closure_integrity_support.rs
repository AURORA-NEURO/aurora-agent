//! Section P32 frontier: compile decision-section closure with explicit omissions and replay proof.
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-section-P32-F01";
pub const CONTRACT_VERSION: &str = "section-local-closure-integrity/1.0";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.section.closure-integrity-card-1+json";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionClaim4 {
    pub claim_id: String,
    pub section_id: String,
    pub statement: String,
    pub evidence_digest: ContentHash,
    pub confidence_basis: String,
    pub local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub unresolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureIntegrityRequest4 {
    pub request_id: String,
    pub purpose: String,
    pub claims: Vec<SectionClaim4>,
    pub required_claim_order: Vec<String>,
    pub required_section_order: Vec<String>,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub adversarial_events: Vec<String>,
    pub action_budget: u32,
    pub action_count: u32,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureIntegrityArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub claim_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureIntegrityCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub mode: String,
    pub scale: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub claim_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub section_order: Vec<String>,
    pub confidence_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub closure_digest: ContentHash,
    pub artifact: ClosureIntegrityArtifact4,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ClosureIntegrityError {
    #[error("invalid closure-integrity request: {0}")]
    Invalid(String),
    #[error("closure-integrity card failed validation: {0}")]
    Output(String),
}

fn ordered(v: &[String]) -> bool { v.windows(2).all(|p| p[0] < p[1]) }
fn digest(v: &ContentHash) -> bool { v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit()) }
fn nonempty(v: &str) -> bool { !v.trim().is_empty() }

pub fn manifest(id: &str, version: &str, scale: &str, mode: &str) -> serde_json::Value {
    json!({"schema_version":"1.0.0","capability_id":id,"version":version,"owner_crate":"section","consumers":["decision-section compiler","context certificate verifier","research workbench","release auditor"],"behavior":format!("compile omission-aware decision-section closure at {scale} ({mode})"),"value":"makes every accepted, rejected, unknown, and omitted claim auditable without silently presenting incomplete evidence","input_schema":"ClosureIntegrityRequest4@1","output_schema":"ClosureIntegrityCard7@1","effects":["emit:closure-card","retain:omission-certificate","block:unsafe-release"],"permissions":["read:local-section-claims"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}

impl ClosureIntegrityCard7 {
    pub fn validate(&self) -> Result<(), ClosureIntegrityError> {
        if self.schema_version != "1.0.0" || !nonempty(&self.contract_version) || !nonempty(&self.feature_id) || !nonempty(&self.mode) || !nonempty(&self.scale) || !nonempty(&self.request_id) || !nonempty(&self.purpose) || self.boundary != BOUNDARY || !self.raw_data_local || !self.aggregate_only || self.claim_order.is_empty() || !ordered(&self.claim_order) || !ordered(&self.accepted_order) || !ordered(&self.rejected_order) || !ordered(&self.unknown_order) || !ordered(&self.omitted_order) || !ordered(&self.section_order) || !ordered(&self.confidence_order) || !ordered(&self.negative_evidence_order) || !digest(&self.replay_identity) || !digest(&self.closure_digest) || self.artifact.content_type != CONTENT_TYPE || self.artifact.content_hash != self.closure_digest || self.artifact.boundary != BOUNDARY {
            return Err(ClosureIntegrityError::Output("section identity, ordering, locality, digest, or artifact is invalid".into()));
        }
        let ids = BTreeSet::from_iter(self.claim_order.iter().cloned());
        let states = self.accepted_order.iter().chain(&self.rejected_order).chain(&self.unknown_order).chain(&self.omitted_order).cloned().collect::<Vec<_>>();
        if ids.len() != self.claim_order.len() || states.len() != ids.len() || BTreeSet::from_iter(states) != ids { return Err(ClosureIntegrityError::Output("claim states do not partition".into())); }
        Ok(())
    }
}

pub fn compile(request: &ClosureIntegrityRequest4, id: &str, version: &str, scale: &str, mode: &str) -> Result<ClosureIntegrityCard7, ClosureIntegrityError> {
    if !nonempty(&request.request_id) || !nonempty(&request.purpose) || request.claims.is_empty() || request.required_claim_order.is_empty() || request.required_section_order.is_empty() || !digest(&request.replay_identity) || request.boundary != BOUNDARY || !request.raw_data_local || !request.aggregate_only || !ordered(&request.required_claim_order) || !ordered(&request.required_section_order) || !ordered(&request.adversarial_events) { return Err(ClosureIntegrityError::Invalid("closure identity, requirements, digest, ordering, locality, or boundary is invalid".into())); }
    let mut rows = request.claims.clone(); rows.sort_by(|a,b| a.claim_id.cmp(&b.claim_id));
    let mut seen = BTreeSet::new(); let mut order = Vec::new(); let mut accepted = BTreeSet::new(); let mut rejected = BTreeSet::new(); let mut unknown = BTreeSet::new(); let mut omitted = BTreeSet::new(); let mut sections = BTreeSet::new(); let mut confidence = BTreeSet::new(); let mut negative = BTreeSet::new(); let mut digests = BTreeSet::new();
    for c in &rows {
        if !seen.insert(c.claim_id.clone()) || !nonempty(&c.claim_id) || !nonempty(&c.section_id) || !nonempty(&c.statement) || !digest(&c.evidence_digest) || !nonempty(&c.confidence_basis) || !c.local || !c.aggregate_only { return Err(ClosureIntegrityError::Invalid("claim identity, evidence, confidence, or locality is invalid".into())); }
        order.push(c.claim_id.clone()); sections.insert(c.section_id.clone()); confidence.insert(c.confidence_basis.clone()); digests.insert(c.evidence_digest.clone()); if c.negative_result { negative.insert(format!("{}:negative-result", c.claim_id)); }
        if c.unresolved { unknown.insert(c.claim_id.clone()); } else if !request.required_claim_order.contains(&c.claim_id) || !request.required_section_order.contains(&c.section_id) { rejected.insert(c.claim_id.clone()); } else if c.evidence_digest == request.replay_identity { omitted.insert(c.claim_id.clone()); } else { accepted.insert(c.claim_id.clone()); }
    }
    let global = !request.policy_allowed || !request.protected_closure || !request.signed_approval || !request.raw_data_local || !request.aggregate_only || !request.adversarial_events.is_empty() || request.action_count > request.action_budget;
    if global { omitted.extend(order.iter().cloned()); accepted.clear(); rejected.clear(); unknown.clear(); }
    let missing = !request.required_claim_order.iter().all(|c| seen.contains(c)) || !request.required_section_order.iter().all(|s| sections.contains(s));
    let disposition = if global { "blocked" } else if missing { "unknown" } else if !rejected.is_empty() || !unknown.is_empty() || !omitted.is_empty() { "partial" } else { "qualified" };
    let mut payload = json!({"schema_version":"1.0.0","contract_version":version,"feature_id":id,"mode":mode,"scale":scale,"request_id":request.request_id,"purpose":request.purpose,"disposition":disposition,"claim_order":order,"accepted_order":accepted.into_iter().collect::<Vec<_>>(),"rejected_order":rejected.into_iter().collect::<Vec<_>>(),"unknown_order":unknown.into_iter().collect::<Vec<_>>(),"omitted_order":omitted.into_iter().collect::<Vec<_>>(),"section_order":sections.into_iter().collect::<Vec<_>>(),"confidence_order":confidence.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":BOUNDARY});
    let hash = ContentHash::of_value(&payload).map_err(|e| ClosureIntegrityError::Output(e.to_string()))?; payload["closure_digest"] = json!(hash); payload["artifact"] = json!({"artifact_id":format!("section-closure:{}", request.request_id),"content_type":CONTENT_TYPE,"content_hash":hash,"semantic_loss":payload["omitted_order"],"claim_digests":digests.into_iter().collect::<Vec<_>>(),"boundary":BOUNDARY}); payload["effect_receipts"] = json!(if disposition == "qualified" { vec![format!("emit:closure-card:{}", request.request_id)] } else { vec!["block:unsafe-release".to_string()] });
    let out: ClosureIntegrityCard7 = serde_json::from_value(payload).map_err(|e| ClosureIntegrityError::Output(e.to_string()))?; out.validate()?; Ok(out)
}
