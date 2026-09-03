//! Deterministic retrieval-synthesis operations services for Worldgen P02 F29-F32.
//!
//! The service is a bounded control-plane capability: it admits a typed retrieval
//! request, accounts for capacity and checkpoints, and emits a replayable receipt.
//! It never performs network I/O or exports raw preclinical data.
use std::collections::BTreeSet;
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use super::retrieval_support::{self, RetrievalQuery};

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.retrieval-operations-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalOperationsRequest {
    pub query: RetrievalQuery,
    pub capacity_units: u64,
    pub requested_event_order: Vec<String>,
    pub completed_event_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub policy_allow: bool,
    pub federation_approved: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalOperationsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub disposition: String,
    pub event_order: Vec<String>,
    pub completed_event_order: Vec<String>,
    pub retryable_event_order: Vec<String>,
    pub dropped_event_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub capacity_units: u64,
    pub used_units: u64,
    pub checkpoint_seq: u64,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub operations_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetrievalOperationsError {
    #[error("invalid retrieval operations request: {0}")]
    Invalid(String),
    #[error("retrieval operations inference failed: {0}")]
    Inference(String),
    #[error("invalid retrieval operations receipt: {0}")]
    Receipt(String),
    #[error("retrieval operations artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool { values.windows(2).all(|p| p[0] < p[1]) }
fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit()) }

impl RetrievalOperationsReceipt {
    pub fn validate(&self) -> Result<(), RetrievalOperationsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|v| v.as_str()) != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|v| v.as_str()) != Some(CONTENT_TYPE)
            || !self.raw_data_local || !self.aggregate_only
            || self.request_id.trim().is_empty() || self.event_order.is_empty()
            || self.effect_receipts.is_empty() || self.capacity_units == 0
            || self.used_units > self.capacity_units
            || ![&self.replay_identity, &self.synthesis_digest, &self.operations_digest].into_iter().all(digest)
        { return Err(RetrievalOperationsError::Receipt("operations identity, locality, capacity, digests, or effects are incomplete".into())); }
        for values in [&self.event_order, &self.completed_event_order, &self.retryable_event_order,
                       &self.dropped_event_order, &self.candidate_order, &self.selected_order,
                       &self.unresolved_order, &self.blocked_order, &self.omissions,
                       &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if !ordered(values) { return Err(RetrievalOperationsError::Receipt("operations ordering is not canonical".into())); }
        }
        let events = self.event_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self.completed_event_order.iter().chain(&self.retryable_event_order).chain(&self.dropped_event_order).cloned().collect::<Vec<_>>();
        if events.len() != self.event_order.len() || states.len() != events.len() || states.iter().cloned().collect::<BTreeSet<_>>() != events {
            return Err(RetrievalOperationsError::Receipt("operation states do not partition requested events".into()));
        }
        let candidates = self.candidate_order.iter().cloned().collect::<BTreeSet<_>>();
        let retrieval_states = self.selected_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).cloned().collect::<Vec<_>>();
        if candidates.len() != self.candidate_order.len() || retrieval_states.len() != candidates.len() || retrieval_states.iter().cloned().collect::<BTreeSet<_>>() != candidates {
            return Err(RetrievalOperationsError::Receipt("retrieval states do not partition candidates".into()));
        }
        if !self.effect_receipts.iter().all(|e| e.starts_with("operate:retrieval-operations:") || e == "block:unsafe-release") {
            return Err(RetrievalOperationsError::Receipt("effect is outside operations gate".into()));
        }
        Ok(())
    }
}

pub fn manifest(feature_id: &str, contract_version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["research program lead","benchmark curator","preclinical neuroscientist","operations steward"],"behavior":format!("operate bounded retrieval synthesis for {scale} with capacity, checkpoint, retry, and policy accounting"),"value":"turns retrieval synthesis into a replayable institution-local operations product without hiding omissions or negative evidence","input_schema":input_schema,"output_schema":"EvidenceSynthesis6@1","effects":["operate:retrieval-operations","block:unsafe-release"],"permissions":["operate:institution-node","read:local-research-artifacts"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":PRECLINICAL_BOUNDARY})
}

pub fn operate(request: &RetrievalOperationsRequest, feature_id: &str, contract_version: &str, _scale: &str, require_federation: bool) -> Result<RetrievalOperationsReceipt, RetrievalOperationsError> {
    if request.boundary != PRECLINICAL_BOUNDARY || request.query.boundary != PRECLINICAL_BOUNDARY
        || !request.query.raw_data_local || !request.query.aggregate_only || request.capacity_units == 0
        || request.requested_event_order.is_empty() || !ordered(&request.requested_event_order)
        || !ordered(&request.completed_event_order) || request.completed_event_order.iter().any(|e| !request.requested_event_order.contains(e)) {
        return Err(RetrievalOperationsError::Invalid("operations boundary, capacity, event order, locality, or checkpoint is invalid".into()));
    }
    let candidate_ids = request.query.candidates.iter().map(|c| c.candidate_id.clone()).collect::<Vec<_>>();
    let mut expected = candidate_ids.clone(); expected.sort(); expected.dedup();
    if expected.len() != candidate_ids.len() || expected != request.requested_event_order { return Err(RetrievalOperationsError::Invalid("requested events must exactly cover canonical candidate ids".into())); }
    let mut bounded_query = request.query.clone(); bounded_query.max_budget_units = bounded_query.max_budget_units.min(request.capacity_units);
    let synthesis = retrieval_support::infer(&bounded_query, feature_id, contract_version).map_err(|e| RetrievalOperationsError::Inference(e.to_string()))?;
    let candidate_set = synthesis.candidate_order.iter().cloned().collect::<BTreeSet<_>>();
    let selected = synthesis.selected_order.iter().cloned().collect::<BTreeSet<_>>();
    let unresolved = synthesis.unresolved_order.iter().cloned().collect::<BTreeSet<_>>();
    let blocked = synthesis.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut completed = selected.clone();
    let mut retryable = unresolved.clone();
    let mut dropped = blocked.clone();
    let mut omissions = synthesis.omission_order.clone();
    let mut uncertainty = synthesis.uncertainty_order.clone();
    let negative = synthesis.negative_evidence_order.clone();
    if !request.policy_allow { omissions.push("request:policy-denied".into()); }
    if require_federation && !request.federation_approved { omissions.push("request:federation-approval-missing".into()); }
    // Checkpoint sequence is explicit receipt metadata; replaying from a checkpoint is
    // deterministic and does not itself downgrade evidence quality.
    // A prior completion is retained only when it is still in the current selected set.
    for id in &request.completed_event_order { if candidate_set.contains(id) && !selected.contains(id) { completed.remove(id); dropped.insert(id.clone()); retryable.remove(id); } }
    let safe_authority = request.policy_allow && request.query.protected_closure && (!require_federation || request.federation_approved);
    let disposition = if !safe_authority { "blocked" } else if completed.len() == candidate_set.len() && omissions.is_empty() && uncertainty.is_empty() && negative.is_empty() { "qualified" } else { "partial" };
    if disposition == "blocked" { completed.clear(); retryable = candidate_set.clone(); dropped.clear(); }
    let mut event_completed = completed.clone(); let mut event_retryable = retryable.clone(); let mut event_dropped = dropped.clone();
    if disposition == "blocked" { event_completed.clear(); event_retryable = candidate_set.clone(); event_dropped.clear(); }
    let effect_receipts = if disposition == "blocked" { vec!["block:unsafe-release".to_string()] } else { vec![format!("operate:retrieval-operations:{}", request.query.request_id)] };
    let mut payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.query.request_id,"disposition":disposition,"event_order":request.requested_event_order,"completed_event_order":event_completed,"retryable_event_order":event_retryable,"dropped_event_order":event_dropped,"candidate_order":synthesis.candidate_order,"selected_order":synthesis.selected_order,"unresolved_order":synthesis.unresolved_order,"blocked_order":synthesis.blocked_order,"capacity_units":request.capacity_units,"used_units":synthesis.total_units,"checkpoint_seq":request.checkpoint_seq,"replay_identity":request.query.replay_identity,"synthesis_digest":synthesis.synthesis_digest,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"effect_receipts":effect_receipts,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let operations_digest = ContentHash::of_value(&payload).map_err(|e| RetrievalOperationsError::Artifact(e.to_string()))?;
    payload["operations_digest"] = json!(operations_digest.as_str());
    let artifact_digest = operations_digest.clone();
    let artifact = json!({"artifact_id":format!("retrieval-operations:{}", request.query.request_id),"content_type":CONTENT_TYPE,"content_hash":artifact_digest,"boundary":PRECLINICAL_BOUNDARY});
    let receipt = RetrievalOperationsReceipt { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:contract_version.into(), feature_id:feature_id.into(), request_id:request.query.request_id.clone(), disposition:disposition.into(), event_order:request.requested_event_order.clone(), completed_event_order:sorted_set(&event_completed), retryable_event_order:sorted_set(&event_retryable), dropped_event_order:sorted_set(&event_dropped), candidate_order:synthesis.candidate_order.clone(), selected_order:synthesis.selected_order.clone(), unresolved_order:synthesis.unresolved_order.clone(), blocked_order:synthesis.blocked_order.clone(), capacity_units:request.capacity_units, used_units:synthesis.total_units.min(request.capacity_units), checkpoint_seq:request.checkpoint_seq, replay_identity:request.query.replay_identity.clone(), synthesis_digest:synthesis.synthesis_digest.clone(), operations_digest, omissions:sorted_vec(omissions), uncertainty:sorted_vec(uncertainty), negative_evidence:negative, effect_receipts, artifact, raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}
fn sorted_set(values: &BTreeSet<String>) -> Vec<String> { values.iter().cloned().collect() }
fn sorted_vec(mut values: Vec<String>) -> Vec<String> { values.sort(); values.dedup(); values }

#[cfg(test)]
mod tests {
    use super::*; use crate::retrieval_support::{RetrievalCandidate, RetrievalQuery};
    fn h(v:&str)->ContentHash { ContentHash::of_bytes(v.as_bytes()) }
    fn request()->RetrievalOperationsRequest { let c=RetrievalCandidate{candidate_id:"candidate:ops".into(),source_id:"source:ops".into(),title:"operations study".into(),study_id:"study:ops".into(),modality:"imaging".into(),relevance_milli:900,freshness_milli:900,evidence_state:"supported".into(),content_digest:h("content"),provenance_digest:h("provenance"),replay_identity:h("replay"),estimated_units:1,permitted:true,comparable:true,negative_result:false}; RetrievalOperationsRequest{query:RetrievalQuery{request_id:"ops:req".into(),researcher:"researcher".into(),corpus_id:"corpus".into(),purpose:"operate retrieval".into(),semantic_profile:"prov-v1".into(),query_terms:vec!["ops".into()],candidates:vec![c],minimum_relevance_milli:500,minimum_freshness_milli:500,max_budget_units:4,replay_identity:h("replay"),policy_allow:true,protected_closure:true,raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()},capacity_units:4,requested_event_order:vec!["candidate:ops".into()],completed_event_order:vec![],checkpoint_seq:1,policy_allow:true,federation_approved:true,boundary:PRECLINICAL_BOUNDARY.into()} }
    #[test] fn local_qualified(){ let r=operate(&request(),"AFA-worldgen-P02-F29","worldgen-local-retrieval-synthesis-operations/1.0","local single-study",false).unwrap(); assert_eq!(r.disposition,"qualified"); }
    #[test] fn policy_blocks(){ let mut q=request(); q.policy_allow=false; let r=operate(&q,"AFA-worldgen-P02-F29","worldgen-local-retrieval-synthesis-operations/1.0","local single-study",false).unwrap(); assert_eq!(r.disposition,"blocked"); }
    #[test] fn federation_gate_blocks(){ let mut q=request(); q.federation_approved=false; let r=operate(&q,"AFA-worldgen-P02-F32","worldgen-federated-continual-retrieval-synthesis-operations/1.0","federated continual/autonomous",true).unwrap(); assert_eq!(r.disposition,"blocked"); }
}
