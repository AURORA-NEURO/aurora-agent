//! Bounded agent automation for Worldgen P02 F09–F12.
//!
//! The copilot can inspect and retain caller-supplied evidence summaries, but it cannot fetch
//! sources, invoke arbitrary tools, move raw data, or make a clinical decision. Approval and
//! federation gates are represented in the receipt so a denied action is never mistaken for a
//! successful run.

use super::retrieval_support::{self, RetrievalQuery, RetrievalReceipt, BOUNDARY, SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.retrieval-copilot-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCopilotRequest {
    pub agent_id: String,
    pub query: RetrievalQuery,
    pub allowed_actions: Vec<String>,
    pub requested_actions: Vec<String>,
    pub action_budget: u64,
    pub dry_run: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub disposition: String,
    pub action_order: Vec<String>,
    pub denied_action_order: Vec<String>,
    pub tool_receipts: Vec<String>,
    pub synthesis: RetrievalReceipt,
    pub copilot_digest: ContentHash,
    pub artifact: serde_json::Value,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalCopilotError {
    #[error("invalid retrieval copilot request: {0}")]
    Invalid(String),
    #[error("invalid retrieval copilot receipt: {0}")]
    Receipt(String),
    #[error("retrieval copilot artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }
fn ordered(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }

impl RetrievalCopilotReceipt {
    pub fn validate(&self) -> Result<(), RetrievalCopilotError> {
        if self.schema_version != SCHEMA_VERSION || self.boundary != BOUNDARY || self.artifact.get("boundary").and_then(|v| v.as_str()) != Some(BOUNDARY) || self.artifact.get("content_type").and_then(|v| v.as_str()) != Some(CONTENT_TYPE) || !self.raw_data_local || !self.aggregate_only || self.request_id.trim().is_empty() || self.agent_id.trim().is_empty() || self.action_order.is_empty() || self.tool_receipts.is_empty() || self.effect_receipts.is_empty() || !digest(&self.copilot_digest) {
            return Err(RetrievalCopilotError::Receipt("retrieval copilot identity, locality, actions, or effects are incomplete".into()));
        }
        for values in [&self.action_order, &self.denied_action_order, &self.tool_receipts, &self.effect_receipts] { if !ordered(values) { return Err(RetrievalCopilotError::Receipt("retrieval copilot ordering is not canonical".into())); } }
        if self.artifact.get("content_hash").and_then(|v| v.as_str()) != Some(self.copilot_digest.as_str()) { return Err(RetrievalCopilotError::Receipt("retrieval copilot artifact digest is inconsistent".into())); }
        self.synthesis.validate().map_err(|error| RetrievalCopilotError::Receipt(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, RetrievalCopilotError> { self.validate()?; let value = serde_json::to_value(self).map_err(|e| RetrievalCopilotError::Receipt(e.to_string()))?; ContentHash::of_value(&value).map_err(|e| RetrievalCopilotError::Receipt(e.to_string())) }
}

pub fn manifest(feature_id: &str, version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value {
    json!({"schema_version":SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["research program lead","preclinical neuroscientist","bioinformatician","imaging core scientist"],"behavior":format!("run a bounded read-only retrieval copilot for {scale}"),"value":"turns typed evidence synthesis into approval-aware, omission-visible agent actions","input_schema":input_schema,"output_schema":"EvidenceSynthesis3@1","effects":["invoke:bounded-retrieval-tool","block:unsafe-release"],"permissions":["read:local-research-artifacts"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":BOUNDARY})
}

pub fn run(request: &RetrievalCopilotRequest, feature_id: &str, contract_version: &str, require_approval: bool, require_federation: bool) -> Result<RetrievalCopilotReceipt, RetrievalCopilotError> {
    if request.agent_id.trim().is_empty() || request.allowed_actions.is_empty() || request.requested_actions.is_empty() || request.action_budget == 0 { return Err(RetrievalCopilotError::Invalid("retrieval copilot agent, actions, or budget is invalid".into())); }
    let synthesis = retrieval_support::infer(&request.query, feature_id, contract_version).map_err(|error| RetrievalCopilotError::Invalid(error.to_string()))?;
    let mut action_order = request.requested_actions.clone(); action_order.sort(); action_order.dedup();
    let denied_action_order = action_order.iter().filter(|action| !request.allowed_actions.contains(action)).cloned().collect::<Vec<_>>();
    let approval_missing = require_approval && !request.signed_approval;
    let federation_missing = require_federation && !request.federation_approved;
    let budget_exceeded = action_order.len() as u64 > request.action_budget;
    let authorized = denied_action_order.is_empty() && !approval_missing && !federation_missing && !budget_exceeded && request.query.policy_allow && request.query.protected_closure;
    let disposition = if !authorized { "blocked" } else if synthesis.disposition == "qualified" { "qualified" } else { "partial" };
    let denied = if authorized { Vec::new() } else { let mut values = denied_action_order.clone(); if approval_missing { values.push("request:signed-approval-missing".into()); } if federation_missing { values.push("request:federation-approval-missing".into()); } if budget_exceeded { values.push("request:action-budget-exceeded".into()); } if !request.query.policy_allow { values.push("request:policy-denied".into()); } if !request.query.protected_closure { values.push("request:protected-closure-incomplete".into()); } values.sort(); values.dedup(); values };
    let tool_receipts = if authorized { vec![format!("tool:bounded-retrieval:{}", request.query.request_id)] } else { vec!["tool:blocked-retrieval".into()] };
    let effects = if disposition == "qualified" && !request.dry_run { vec![format!("invoke:bounded-retrieval-tool:{}", request.query.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version":SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.query.request_id,"agent_id":request.agent_id,"disposition":disposition,"action_order":action_order,"denied_action_order":denied,"tool_receipts":tool_receipts,"synthesis_digest":synthesis.synthesis_digest,"dry_run":request.dry_run,"replay_identity":request.query.replay_identity,"effect_receipts":effects,"raw_data_local":true,"aggregate_only":true,"boundary":BOUNDARY});
    let copilot_digest = ContentHash::of_value(&payload).map_err(|e| RetrievalCopilotError::Artifact(e.to_string()))?;
    let receipt = RetrievalCopilotReceipt { schema_version:SCHEMA_VERSION.into(), contract_version:contract_version.into(), feature_id:feature_id.into(), request_id:request.query.request_id.clone(), agent_id:request.agent_id.clone(), disposition:disposition.into(), action_order:payload["action_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), denied_action_order:sorted(&payload["denied_action_order"]), tool_receipts:sorted(&payload["tool_receipts"]), synthesis, copilot_digest:copilot_digest.clone(), artifact:json!({"artifact_id":format!("retrieval-copilot:{}",request.query.request_id),"content_type":CONTENT_TYPE,"content_hash":copilot_digest,"boundary":BOUNDARY}), effect_receipts:sorted(&payload["effect_receipts"]), raw_data_local:true, aggregate_only:true, boundary:BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

fn sorted(value: &serde_json::Value) -> Vec<String> { let mut output = value.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect::<Vec<_>>(); output.sort(); output.dedup(); output }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_support::{RetrievalCandidate, RetrievalQuery};

    fn hash(seed: &str) -> ContentHash { ContentHash::of_bytes(seed.as_bytes()) }
    fn query() -> RetrievalQuery {
        let candidate = RetrievalCandidate { candidate_id:"candidate:a".into(), source_id:"source:a".into(), title:"organoid study".into(), study_id:"study:a".into(), modality:"imaging".into(), relevance_milli:900, freshness_milli:900, evidence_state:"supported".into(), content_digest:hash("content"), provenance_digest:hash("provenance"), replay_identity:hash("replay"), estimated_units:1, permitted:true, comparable:true, negative_result:false };
        RetrievalQuery { request_id:"copilot:req".into(), researcher:"researcher:a".into(), corpus_id:"corpus:local".into(), purpose:"inspect synaptic evidence".into(), semantic_profile:"prov-v1".into(), query_terms:vec!["synapse".into()], candidates:vec![candidate], minimum_relevance_milli:500, minimum_freshness_milli:500, max_budget_units:4, replay_identity:hash("replay"), policy_allow:true, protected_closure:true, raw_data_local:true, aggregate_only:true, boundary:BOUNDARY.into() }
    }
    fn request() -> RetrievalCopilotRequest { RetrievalCopilotRequest { agent_id:"agent:research-copilot".into(), query:query(), allowed_actions:vec!["read:local-evidence".into(),"inspect:omissions".into()], requested_actions:vec!["read:local-evidence".into()], action_budget:2, dry_run:false, signed_approval:true, federation_approved:true } }
    #[test] fn local_copilot_invokes_only_allowed_bounded_tool() { let receipt=run(&request(),"AFA-worldgen-P02-F09","worldgen-local-retrieval-synthesis-copilot/1.0",false,false).unwrap(); assert_eq!(receipt.disposition,"qualified"); assert!(receipt.effect_receipts[0].starts_with("invoke:bounded-retrieval-tool:")); assert!(receipt.denied_action_order.is_empty()); }
    #[test] fn multimodal_copilot_requires_signed_approval() { let mut req=request(); req.signed_approval=false; let receipt=run(&req,"AFA-worldgen-P02-F10","worldgen-multimodal-retrieval-synthesis-copilot/1.0",true,false).unwrap(); assert_eq!(receipt.disposition,"blocked"); assert!(receipt.denied_action_order.iter().any(|v|v.contains("signed-approval"))); }
    #[test] fn federated_copilot_requires_federation_approval() { let mut req=request(); req.federation_approved=false; let receipt=run(&req,"AFA-worldgen-P02-F12","worldgen-federated-continual-retrieval-synthesis-copilot/1.0",true,true).unwrap(); assert_eq!(receipt.disposition,"blocked"); assert!(receipt.denied_action_order.iter().any(|v|v.contains("federation-approval"))); }
    #[test] fn disallowed_action_is_never_invoked() { let mut req=request(); req.requested_actions.push("write:external-system".into()); let receipt=run(&req,"AFA-worldgen-P02-F09","worldgen-local-retrieval-synthesis-copilot/1.0",false,false).unwrap(); assert_eq!(receipt.disposition,"blocked"); assert!(receipt.denied_action_order.contains(&"write:external-system".into())); }
}
