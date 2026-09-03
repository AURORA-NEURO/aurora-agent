//! Resumable retrieval-synthesis workflow fabric for Worldgen P02 F13–F16.
//!
//! The fabric schedules only the bounded copilot plan; it does not execute code, contact
//! instruments, fetch sources, or move raw data. Checkpoints, compensation, budgets, and replay
//! identity are first-class product state.

use super::retrieval_copilot_support::{self, RetrievalCopilotReceipt, RetrievalCopilotRequest};
use super::retrieval_support::{BOUNDARY, SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.retrieval-workflow-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalWorkflowRequest {
    pub workflow_id: String,
    pub copilot: RetrievalCopilotRequest,
    pub stage_order: Vec<String>,
    pub completed_stage_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub budget_units: u64,
    pub compensation_enabled: bool,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub workflow_id: String,
    pub disposition: String,
    pub stage_order: Vec<String>,
    pub completed_stage_order: Vec<String>,
    pub pending_stage_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub budget_units: u64,
    pub consumed_units: u64,
    pub replay_identity: ContentHash,
    pub copilot: RetrievalCopilotReceipt,
    pub workflow_digest: ContentHash,
    pub artifact: serde_json::Value,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalWorkflowError {
    #[error("invalid retrieval workflow request: {0}")]
    Invalid(String),
    #[error("invalid retrieval workflow receipt: {0}")]
    Receipt(String),
    #[error("retrieval workflow artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }
fn ordered(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn sorted(value: &serde_json::Value) -> Vec<String> { let mut output=value.as_array().unwrap().iter().map(|v|v.as_str().unwrap().to_owned()).collect::<Vec<_>>(); output.sort(); output.dedup(); output }

impl RetrievalWorkflowReceipt {
    pub fn validate(&self) -> Result<(), RetrievalWorkflowError> {
        if self.schema_version != SCHEMA_VERSION || self.boundary != BOUNDARY || self.artifact.get("boundary").and_then(|v|v.as_str()) != Some(BOUNDARY) || self.artifact.get("content_type").and_then(|v|v.as_str()) != Some(CONTENT_TYPE) || !self.raw_data_local || !self.aggregate_only || self.workflow_id.trim().is_empty() || self.stage_order.is_empty() || self.stage_order.len() != self.completed_stage_order.len() + self.pending_stage_order.len() || self.effect_receipts.is_empty() || !digest(&self.replay_identity) || !digest(&self.workflow_digest) { return Err(RetrievalWorkflowError::Receipt("retrieval workflow identity, stages, locality, or effects are incomplete".into())); }
        for values in [&self.stage_order,&self.completed_stage_order,&self.pending_stage_order,&self.compensation_order,&self.effect_receipts] { if !ordered(values) { return Err(RetrievalWorkflowError::Receipt("retrieval workflow ordering is not canonical".into())); } }
        let stages=self.stage_order.iter().cloned().collect::<BTreeSet<_>>(); let parts=self.completed_stage_order.iter().chain(&self.pending_stage_order).cloned().collect::<Vec<_>>(); if stages.len()!=self.stage_order.len() || parts.len()!=stages.len() || parts.iter().cloned().collect::<BTreeSet<_>>()!=stages { return Err(RetrievalWorkflowError::Receipt("retrieval workflow stages do not partition".into())); }
        if self.copilot.validate().is_err() { return Err(RetrievalWorkflowError::Receipt("nested retrieval copilot receipt is invalid".into())); }
        if self.artifact.get("content_hash").and_then(|v|v.as_str()) != Some(self.workflow_digest.as_str()) { return Err(RetrievalWorkflowError::Receipt("retrieval workflow artifact digest is inconsistent".into())); }
        Ok(())
    }
    pub fn digest(&self)->Result<ContentHash,RetrievalWorkflowError>{self.validate()?;let v=serde_json::to_value(self).map_err(|e|RetrievalWorkflowError::Receipt(e.to_string()))?;ContentHash::of_value(&v).map_err(|e|RetrievalWorkflowError::Receipt(e.to_string()))}
}

pub fn manifest(feature_id:&str,version:&str,input_schema:&str,scale:&str,autonomy:&str)->serde_json::Value{json!({"schema_version":SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["preclinical neuroscientist","bioinformatician","imaging core scientist","benchmark curator"],"behavior":format!("orchestrate a resumable retrieval-synthesis workflow for {scale} with checkpoints and compensation"),"value":"makes retrieval plans restartable, budgeted, and replayable without hidden effects","input_schema":input_schema,"output_schema":"EvidenceSynthesis4@1","effects":["schedule:local-retrieval-workflow","block:unsafe-release"],"permissions":["read:local-research-artifacts"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":BOUNDARY})}

pub fn schedule(request:&RetrievalWorkflowRequest,feature_id:&str,contract_version:&str,require_approval:bool,require_federation:bool)->Result<RetrievalWorkflowReceipt,RetrievalWorkflowError>{
    if request.workflow_id.trim().is_empty()||request.stage_order.is_empty()||request.budget_units==0||request.checkpoint_seq==0||!request.compensation_enabled||!digest(&request.replay_identity)||request.copilot.query.replay_identity!=request.replay_identity { return Err(RetrievalWorkflowError::Invalid("retrieval workflow identity, stages, checkpoint, budget, compensation, or replay is invalid".into())); }
    let mut stages=request.stage_order.clone(); stages.sort(); stages.dedup(); if stages.len()!=request.stage_order.len()||request.completed_stage_order.iter().any(|stage|!stages.contains(stage)){return Err(RetrievalWorkflowError::Invalid("retrieval workflow stages must be unique and declared".into()));}
    let copilot=retrieval_copilot_support::run(&request.copilot,feature_id,contract_version,require_approval,require_federation).map_err(|e|RetrievalWorkflowError::Invalid(e.to_string()))?;
    let completed=request.completed_stage_order.iter().cloned().collect::<BTreeSet<_>>(); let pending=stages.iter().filter(|stage|!completed.contains(*stage)).cloned().collect::<Vec<_>>(); let consumed=request.copilot.query.candidates.iter().map(|candidate|candidate.estimated_units).sum::<u64>().min(request.budget_units); let mut compensation=Vec::new(); if copilot.disposition=="blocked"||consumed>request.budget_units { compensation.push(format!("workflow:{}:retain-partial-artifact",request.workflow_id)); }
    let disposition=if copilot.disposition=="blocked"||compensation.len()>0{"blocked"}else if pending.is_empty(){"qualified"}else{"partial"}; let effects=if disposition=="qualified"{vec![format!("schedule:local-retrieval-workflow:{}",request.workflow_id)]}else{vec!["block:unsafe-release".into()]}; let payload=json!({"schema_version":SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"workflow_id":request.workflow_id,"disposition":disposition,"stage_order":stages,"completed_stage_order":completed,"pending_stage_order":pending,"compensation_order":compensation,"checkpoint_seq":request.checkpoint_seq,"budget_units":request.budget_units,"consumed_units":consumed,"replay_identity":request.replay_identity,"copilot_digest":copilot.copilot_digest,"effect_receipts":effects,"raw_data_local":true,"aggregate_only":true,"boundary":BOUNDARY}); let workflow_digest=ContentHash::of_value(&payload).map_err(|e|RetrievalWorkflowError::Artifact(e.to_string()))?; let receipt=RetrievalWorkflowReceipt{schema_version:SCHEMA_VERSION.into(),contract_version:contract_version.into(),feature_id:feature_id.into(),workflow_id:request.workflow_id.clone(),disposition:disposition.into(),stage_order:sorted(&payload["stage_order"]),completed_stage_order:sorted(&payload["completed_stage_order"]),pending_stage_order:sorted(&payload["pending_stage_order"]),compensation_order:sorted(&payload["compensation_order"]),checkpoint_seq:request.checkpoint_seq,budget_units:request.budget_units,consumed_units:consumed,replay_identity:request.replay_identity.clone(),copilot,workflow_digest:workflow_digest.clone(),artifact:json!({"artifact_id":format!("retrieval-workflow:{}",request.workflow_id),"content_type":CONTENT_TYPE,"content_hash":workflow_digest,"boundary":BOUNDARY}),effect_receipts:sorted(&payload["effect_receipts"]),raw_data_local:true,aggregate_only:true,boundary:BOUNDARY.into()}; receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_copilot_support::RetrievalCopilotRequest;
    use crate::retrieval_support::{RetrievalCandidate, RetrievalQuery};

    fn hash(seed: &str) -> ContentHash { ContentHash::of_bytes(seed.as_bytes()) }
    fn request() -> RetrievalWorkflowRequest {
        let replay = hash("replay");
        let candidate = RetrievalCandidate { candidate_id:"candidate:a".into(), source_id:"source:a".into(), title:"organoid study".into(), study_id:"study:a".into(), modality:"imaging".into(), relevance_milli:900, freshness_milli:900, evidence_state:"supported".into(), content_digest:hash("content"), provenance_digest:hash("provenance"), replay_identity:replay.clone(), estimated_units:1, permitted:true, comparable:true, negative_result:false };
        let query = RetrievalQuery { request_id:"workflow:req".into(), researcher:"researcher:a".into(), corpus_id:"corpus:local".into(), purpose:"inspect evidence".into(), semantic_profile:"prov-v1".into(), query_terms:vec!["organoid".into()], candidates:vec![candidate], minimum_relevance_milli:500, minimum_freshness_milli:500, max_budget_units:4, replay_identity:replay.clone(), policy_allow:true, protected_closure:true, raw_data_local:true, aggregate_only:true, boundary:BOUNDARY.into() };
        RetrievalWorkflowRequest { workflow_id:"workflow:a".into(), copilot:RetrievalCopilotRequest { agent_id:"agent:a".into(), query, allowed_actions:vec!["read:local-evidence".into()], requested_actions:vec!["read:local-evidence".into()], action_budget:2, dry_run:false, signed_approval:true, federation_approved:true }, stage_order:vec!["audit".into(),"retrieve".into(),"synthesize".into()], completed_stage_order:vec!["audit".into(),"retrieve".into(),"synthesize".into()], checkpoint_seq:1, budget_units:5, compensation_enabled:true, replay_identity:replay }
    }
    #[test] fn completed_workflow_schedules_replayable_effect() { let receipt=schedule(&request(),"AFA-worldgen-P02-F13","worldgen-local-retrieval-synthesis-workflow/1.0",false,false).unwrap(); assert_eq!(receipt.disposition,"qualified"); assert_eq!(receipt.pending_stage_order.len(),0); assert!(receipt.effect_receipts[0].starts_with("schedule:local-retrieval-workflow:")); assert!(receipt.digest().is_ok()); }
    #[test] fn partial_workflow_retains_pending_stages() { let mut req=request(); req.completed_stage_order=vec!["retrieve".into()]; let receipt=schedule(&req,"AFA-worldgen-P02-F13","worldgen-local-retrieval-synthesis-workflow/1.0",false,false).unwrap(); assert_eq!(receipt.disposition,"partial"); assert!(receipt.pending_stage_order.contains(&"audit".into())); }
    #[test] fn approval_failure_compensates_and_blocks() { let mut req=request(); req.copilot.signed_approval=false; let receipt=schedule(&req,"AFA-worldgen-P02-F14","worldgen-multimodal-retrieval-synthesis-workflow/1.0",true,false).unwrap(); assert_eq!(receipt.disposition,"blocked"); assert!(!receipt.compensation_order.is_empty()); assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]); }
}
