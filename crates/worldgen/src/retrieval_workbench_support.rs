//! Deterministic researcher workbench over the bounded retrieval copilots (P02 F17–F20).
//!
//! The workbench is a read-only product surface: it renders evidence, omission, provenance,
//! negative-result, and blocked-state panels while retaining the copilot's typed partitions.
//! It never fetches sources, invokes arbitrary tools, moves raw observations, or turns an
//! incomplete synthesis into a scientific or clinical conclusion.

use std::collections::BTreeSet;

use super::retrieval_copilot_support::{self, RetrievalCopilotRequest};
use bioprism_foundation::{TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.retrieval-workbench-receipt+json";
pub const VIEWS: [&str; 4] = ["view:overview", "view:evidence", "view:omissions", "view:provenance"];
pub const PANELS: [&str; 4] = ["panel:negative", "panel:provenance", "panel:qualified", "panel:unknown"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalWorkbenchRequest {
    pub copilot: RetrievalCopilotRequest,
    pub workspace_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u64,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub disposition: String,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub denied_action_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetrievalWorkbenchError {
    #[error("invalid retrieval workbench request: {0}")]
    Invalid(String),
    #[error("retrieval workbench copilot failed: {0}")]
    Copilot(String),
    #[error("retrieval workbench artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }
fn ordered(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn canonical(values: &[&str]) -> Vec<String> { values.iter().map(|value| (*value).to_string()).collect() }

impl RetrievalWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), RetrievalWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.boundary != PRECLINICAL_BOUNDARY || self.artifact.boundary != PRECLINICAL_BOUNDARY || self.artifact.content_type != CONTENT_TYPE || !self.raw_data_local || !self.aggregate_only || self.request_id.trim().is_empty() || self.workspace_id.trim().is_empty() || self.scope.trim().is_empty() || self.view_order != canonical(&VIEWS) || self.panel_order != canonical(&PANELS) || self.candidate_order.is_empty() || self.effect_receipts.is_empty() || !digest(&self.replay_identity) || !digest(&self.copilot_digest) || !digest(&self.workbench_digest) {
            return Err(RetrievalWorkbenchError::Invalid("workbench identity, views, candidates, locality, digests, or effects are incomplete".into()));
        }
        for values in [&self.candidate_order, &self.selected_order, &self.unresolved_order, &self.blocked_order, &self.denied_action_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if !ordered(values) { return Err(RetrievalWorkbenchError::Invalid("workbench ordering is not canonical".into())); }
        }
        let classified = self.selected_order.iter().chain(self.unresolved_order.iter()).chain(self.blocked_order.iter()).cloned().collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect::<BTreeSet<_>>() { return Err(RetrievalWorkbenchError::Invalid("workbench candidate states do not partition".into())); }
        if self.effect_receipts.iter().any(|effect| effect != "block:unsafe-release" && !effect.starts_with("view:retrieval-workbench:")) { return Err(RetrievalWorkbenchError::Invalid("workbench effect is outside read-only gate".into())); }
        self.artifact.validate_metadata().map_err(|error| RetrievalWorkbenchError::Artifact(error.to_string()))
    }
}

pub fn manifest(feature_id: &str, version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["preclinical researcher","research program lead","bioinformatician","imaging core scientist"],"behavior":format!("render a deterministic read-only retrieval workbench for {scale}"),"value":"makes qualified, unknown, omitted, negative, and provenance state directly inspectable without upgrading incomplete evidence","input_schema":input_schema,"output_schema":"EvidenceSynthesis4@1","effects":["view:retrieval-workbench","block:unsafe-release"],"permissions":["read:local-research-artifacts"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":PRECLINICAL_BOUNDARY,"contract_version":version})
}

pub fn render(request: &RetrievalWorkbenchRequest, feature_id: &str, contract_version: &str, require_approval: bool, require_federation: bool) -> Result<RetrievalWorkbenchReceipt, RetrievalWorkbenchError> {
    if request.workspace_id.trim().is_empty() || request.scope.trim().is_empty() || request.budget_units == 0 || request.boundary != PRECLINICAL_BOUNDARY || !request.copilot.dry_run || !request.copilot.query.raw_data_local || !request.copilot.query.aggregate_only || request.requested_view_order != canonical(&VIEWS) || request.requested_panel_order != canonical(&PANELS) || !digest(&request.replay_identity) || request.replay_identity != request.copilot.query.replay_identity { return Err(RetrievalWorkbenchError::Invalid("workbench identity, read-only, budget, locality, views, panels, or replay is invalid".into())); }
    let copilot = retrieval_copilot_support::run(&request.copilot, feature_id, contract_version, require_approval, require_federation).map_err(|error| RetrievalWorkbenchError::Copilot(error.to_string()))?;
    let view_order = canonical(&VIEWS); let panel_order = canonical(&PANELS);
    let candidate_order = copilot.synthesis.candidate_order.clone(); let selected_order = copilot.synthesis.selected_order.clone(); let unresolved_order = copilot.synthesis.unresolved_order.clone(); let blocked_order = copilot.synthesis.blocked_order.clone();
    let copilot_value = serde_json::to_value(&copilot).map_err(|error| RetrievalWorkbenchError::Artifact(error.to_string()))?;
    let copilot_digest = ContentHash::of_value(&copilot_value).map_err(|error| RetrievalWorkbenchError::Artifact(error.to_string()))?;
    let workbench_digest = ContentHash::of_value(&json!({"feature_id":feature_id,"workspace_id":request.workspace_id,"scope":request.scope,"view_order":view_order,"panel_order":panel_order,"candidate_order":candidate_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"replay_identity":request.replay_identity,"copilot_digest":copilot_digest})).map_err(|error| RetrievalWorkbenchError::Artifact(error.to_string()))?;
    let mut omissions = copilot.synthesis.omission_order.clone(); omissions.extend(copilot.denied_action_order.iter().map(|value| format!("action-denied:{value}"))); omissions.push("workbench:read-only-local-view".into()); omissions.sort(); omissions.dedup();
    let mut uncertainty = copilot.synthesis.uncertainty_order.clone(); uncertainty.sort(); uncertainty.dedup();
    let mut negative_evidence = copilot.synthesis.negative_evidence_order.clone(); negative_evidence.sort(); negative_evidence.dedup();
    let effect_receipts = vec![format!("view:retrieval-workbench:{}", request.workspace_id)];
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.copilot.query.request_id,"workspace_id":request.workspace_id,"scope":request.scope,"disposition":copilot.disposition,"view_order":view_order,"panel_order":panel_order,"candidate_order":candidate_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"denied_action_order":copilot.denied_action_order,"replay_identity":request.replay_identity,"copilot_digest":copilot_digest,"workbench_digest":workbench_digest,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative_evidence,"effect_receipts":effect_receipts,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("worldgen-retrieval-workbench:{}", request.workspace_id), CONTENT_TYPE, &payload, vec![], vec![]).map_err(|error| RetrievalWorkbenchError::Artifact(error.to_string()))?;
    let receipt = RetrievalWorkbenchReceipt { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:contract_version.into(), feature_id:feature_id.into(), request_id:request.copilot.query.request_id.clone(), workspace_id:request.workspace_id.clone(), scope:request.scope.clone(), disposition:copilot.disposition.clone(), view_order, panel_order, candidate_order, selected_order, unresolved_order, blocked_order, denied_action_order:copilot.denied_action_order.clone(), replay_identity:request.replay_identity.clone(), copilot_digest, workbench_digest, omissions, uncertainty, negative_evidence, effect_receipts, artifact, raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*; use crate::retrieval_support::{RetrievalCandidate, RetrievalQuery};
    fn hash(seed:&str)->ContentHash{ContentHash::of_bytes(seed.as_bytes())}
    fn request()->RetrievalWorkbenchRequest { let candidate=RetrievalCandidate{candidate_id:"candidate:workbench".into(),source_id:"source:workbench".into(),title:"organoid evidence".into(),study_id:"study:workbench".into(),modality:"imaging".into(),relevance_milli:900,freshness_milli:900,evidence_state:"supported".into(),content_digest:hash("content"),provenance_digest:hash("provenance"),replay_identity:hash("replay"),estimated_units:1,permitted:true,comparable:true,negative_result:false}; let query=RetrievalQuery{request_id:"workbench:req".into(),researcher:"researcher:workbench".into(),corpus_id:"corpus:local".into(),purpose:"inspect evidence".into(),semantic_profile:"prov-v1".into(),query_terms:vec!["organoid".into()],candidates:vec![candidate],minimum_relevance_milli:500,minimum_freshness_milli:500,max_budget_units:4,replay_identity:hash("replay"),policy_allow:true,protected_closure:true,raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()}; RetrievalWorkbenchRequest{copilot:RetrievalCopilotRequest{agent_id:"agent:workbench".into(),query,allowed_actions:vec!["read:local-evidence".into()],requested_actions:vec!["read:local-evidence".into()],action_budget:2,dry_run:true,signed_approval:true,federation_approved:true},workspace_id:"workspace:workbench".into(),scope:"study:workbench".into(),requested_view_order:canonical(&VIEWS),requested_panel_order:canonical(&PANELS),budget_units:4,replay_identity:hash("replay"),boundary:PRECLINICAL_BOUNDARY.into()} }
    #[test] fn read_only_workbench_is_deterministic(){let r=render(&request(),"AFA-worldgen-P02-F17","worldgen-local-retrieval-synthesis-workbench/1.0",false,false).unwrap();assert_eq!(r.disposition,"qualified");assert!(r.effect_receipts[0].starts_with("view:retrieval-workbench:"));}
    #[test] fn policy_denial_stays_visible(){let mut r=request();r.copilot.query.policy_allow=false;let out=render(&r,"AFA-worldgen-P02-F17","worldgen-local-retrieval-synthesis-workbench/1.0",false,false).unwrap();assert_eq!(out.disposition,"blocked");assert!(out.omissions.iter().any(|v|v.contains("denied")));}
    #[test] fn replay_mismatch_is_rejected(){let mut r=request();r.replay_identity=hash("different");assert!(render(&r,"AFA-worldgen-P02-F17","worldgen-local-retrieval-synthesis-workbench/1.0",false,false).is_err());}
    #[test] fn approval_failure_is_rendered_as_blocked(){let mut r=request();r.copilot.signed_approval=false;let out=render(&r,"AFA-worldgen-P02-F18","worldgen-multimodal-retrieval-synthesis-workbench/1.0",true,false).unwrap();assert_eq!(out.disposition,"blocked");assert!(out.denied_action_order.iter().any(|v|v.contains("approval")));}
}
