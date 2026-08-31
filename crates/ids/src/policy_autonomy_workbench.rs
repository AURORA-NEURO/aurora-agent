//! Federated continual policy/autonomy researcher workbench (`AFA-ids-P19-F20`).
//!
//! It gives researchers a typed, replayable view of which bounded actions are admitted, require
//! approval, or are denied. It never performs an action, contacts an institution, or makes a
//! clinical decision.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P19-F20";
pub const CONTRACT_VERSION: &str = "ids-federated-continual-policy-autonomy-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ActionAndAuthority4@1";
pub const OUTPUT_SCHEMA: &str = "PolicyReceipt5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.policy-receipt-5+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionEvidenceState { Proven, Supported, Unknown, Unmeasured, Contradicted, Negative }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionAndAuthority4 {
    pub action_id: String,
    pub actor: String,
    pub requested_effect: String,
    pub scope: String,
    pub autonomy_tier: String,
    pub resource_budget: u64,
    pub resource_cost: u64,
    pub expires_at: u64,
    pub revoked: bool,
    pub evidence_state: ActionEvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionAndAuthorityRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub scope: String,
    pub allowed_effect_order: Vec<String>,
    pub max_autonomy_tier: String,
    pub now_epoch: u64,
    pub actions: Vec<ActionAndAuthority4>,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyReceiptArtifact5 { pub artifact_id: String, pub content_type: String, pub content_hash: ContentHash, pub semantic_loss: Vec<String>, pub provenance_digests: Vec<ContentHash>, pub boundary: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyReceipt5 {
    pub schema_version: String, pub contract_version: String, pub feature_id: String, pub request_id: String, pub scope: String, pub max_autonomy_tier: String, pub disposition: String,
    pub action_order: Vec<String>, pub admitted_order: Vec<String>, pub approval_required_order: Vec<String>, pub denied_order: Vec<String>, pub revoked_order: Vec<String>, pub over_budget_order: Vec<String>, pub scope_mismatch_order: Vec<String>, pub missing_authority_order: Vec<String>, pub omission_order: Vec<String>, pub uncertainty_order: Vec<String>, pub negative_evidence_order: Vec<String>, pub replay_identity: ContentHash, pub receipt_digest: ContentHash, pub artifact: PolicyReceiptArtifact5, pub effect_receipts: Vec<String>, pub raw_data_local: bool, pub aggregate_only: bool, pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PolicyAutonomyWorkbenchError { #[error("invalid action/authority request: {0}")] Invalid(String), #[error("policy receipt failed validation: {0}")] Output(String) }

fn nonempty(value: &str) -> bool { !value.trim().is_empty() }
fn ordered(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }
fn tier(value: &str) -> Option<u8> { value.strip_prefix('a')?.parse().ok().filter(|value: &u8| *value <= 4) }

pub fn policy_autonomy_workbench_manifest() -> serde_json::Value { json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["formal methods researcher","researcher","autonomy policy steward"],"behavior":"present typed bounded-action authorization, revocation, budget, and approval states","value":"makes autonomy boundaries auditable without silently granting effects","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["view:authorized-research-state"],"permissions":["view:authorized-research-state"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}) }

impl PolicyReceipt5 {
    pub fn validate(&self) -> Result<(), PolicyAutonomyWorkbenchError> {
        if self.schema_version != "aurora-research-contract/1.0" || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.boundary != PRECLINICAL_BOUNDARY || self.artifact.boundary != PRECLINICAL_BOUNDARY || self.artifact.content_type != CONTENT_TYPE || !self.raw_data_local || !self.aggregate_only || !nonempty(&self.request_id) || !nonempty(&self.scope) || tier(&self.max_autonomy_tier).is_none() || self.action_order.is_empty() || self.effect_receipts.is_empty() || !["qualified","unresolved","blocked"].contains(&self.disposition.as_str()) { return Err(PolicyAutonomyWorkbenchError::Output("policy identity, locality, tier, actions, or effects are incomplete".into())); }
        for values in [&self.action_order,&self.admitted_order,&self.approval_required_order,&self.denied_order,&self.revoked_order,&self.over_budget_order,&self.scope_mismatch_order,&self.missing_authority_order,&self.omission_order,&self.uncertainty_order,&self.negative_evidence_order,&self.effect_receipts] { if !ordered(values) { return Err(PolicyAutonomyWorkbenchError::Output("policy ordering is not canonical".into())); } }
        let all=self.action_order.iter().cloned().collect::<BTreeSet<_>>(); let states=self.admitted_order.iter().chain(&self.approval_required_order).chain(&self.denied_order).cloned().collect::<Vec<_>>(); if all.len()!=self.action_order.len() || states.len()!=all.len() || BTreeSet::from_iter(states)!=all { return Err(PolicyAutonomyWorkbenchError::Output("action states do not partition".into())); }
        if !digest(&self.replay_identity) || !digest(&self.receipt_digest) || self.artifact.content_hash!=self.receipt_digest || self.artifact.provenance_digests.iter().any(|value|!digest(value)) { return Err(PolicyAutonomyWorkbenchError::Output("policy digest or artifact metadata is invalid".into())); }
        if self.effect_receipts.iter().any(|effect| effect != "block:unsafe-release" && !effect.starts_with("view:authorized-research-state:")) { return Err(PolicyAutonomyWorkbenchError::Output("effect is outside researcher-view gate".into())); }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, PolicyAutonomyWorkbenchError> { self.validate()?; ContentHash::of_value(&serde_json::to_value(self).map_err(|error|PolicyAutonomyWorkbenchError::Output(error.to_string()))?).map_err(|error|PolicyAutonomyWorkbenchError::Output(error.to_string())) }
}

fn validate_request(request: &ActionAndAuthorityRequest4) -> Result<(), PolicyAutonomyWorkbenchError> { if request.schema_version!=INPUT_SCHEMA || !nonempty(&request.request_id) || !nonempty(&request.scope) || request.allowed_effect_order.is_empty() || !ordered(&request.allowed_effect_order) || tier(&request.max_autonomy_tier).is_none() || request.actions.is_empty() || !digest(&request.replay_identity) || !ordered(&request.adversarial_events) || request.boundary!=PRECLINICAL_BOUNDARY || !request.raw_data_local || !request.aggregate_only { return Err(PolicyAutonomyWorkbenchError::Invalid("request identity, effect order, tier, digests, locality, or boundary is invalid".into())); } let mut ids=BTreeSet::new(); for action in &request.actions { if !nonempty(&action.action_id) || !nonempty(&action.actor) || !nonempty(&action.requested_effect) || !nonempty(&action.scope) || tier(&action.autonomy_tier).is_none() || !digest(&action.provenance_digest) || !digest(&action.replay_identity) || !ordered(&action.omission_order) || !ids.insert(action.action_id.clone()) { return Err(PolicyAutonomyWorkbenchError::Invalid("action identity, tier, digests, ordering, or uniqueness is invalid".into())); } } Ok(()) }

pub fn operate_policy_autonomy(request: &ActionAndAuthorityRequest4) -> Result<PolicyReceipt5, PolicyAutonomyWorkbenchError> { validate_request(request)?; let mut actions=request.actions.clone(); actions.sort_by(|left,right|left.action_id.cmp(&right.action_id)); let action_order=actions.iter().map(|item|item.action_id.clone()).collect::<Vec<_>>(); let max=tier(&request.max_autonomy_tier).unwrap(); let mut admitted=BTreeSet::new();let mut approval=BTreeSet::new();let mut denied=BTreeSet::new();let mut revoked=BTreeSet::new();let mut over_budget=BTreeSet::new();let mut scope_mismatch=BTreeSet::new();let mut missing_authority=BTreeSet::new();let mut omission=BTreeSet::new();let mut uncertainty=BTreeSet::new();let mut negative=BTreeSet::new();let mut provenance=BTreeSet::new(); for action in &actions { let id=action.action_id.clone(); provenance.insert(action.provenance_digest.clone()); omission.extend(action.omission_order.iter().map(|value|format!("{id}:{value}"))); if action.negative_result || action.evidence_state==ActionEvidenceState::Negative { negative.insert(format!("{id}:negative-result")); } if action.revoked { revoked.insert(id.clone()); omission.insert(format!("{id}:revoked")); } else if action.scope!=request.scope { scope_mismatch.insert(id.clone()); omission.insert(format!("{id}:scope-mismatch")); } else if !request.allowed_effect_order.contains(&action.requested_effect) { denied.insert(id.clone()); missing_authority.insert(format!("{id}:effect-not-authorized")); } else if tier(&action.autonomy_tier).unwrap()>max { approval.insert(id.clone()); uncertainty.insert(format!("{id}:autonomy-tier-approval-required")); } else if action.resource_cost>action.resource_budget || action.resource_cost>request.now_epoch.saturating_add(action.expires_at) { over_budget.insert(id.clone()); omission.insert(format!("{id}:budget-or-expiry")); } else if action.expires_at<request.now_epoch { denied.insert(id.clone()); omission.insert(format!("{id}:expired")); } else if action.replay_identity!=request.replay_identity || !action.local || !action.aggregate_only { missing_authority.insert(format!("{id}:replay-or-locality")); approval.insert(id.clone()); } else if !matches!(action.evidence_state,ActionEvidenceState::Proven|ActionEvidenceState::Supported) { approval.insert(id.clone()); uncertainty.insert(format!("{id}:evidence-not-supported")); } else { admitted.insert(id); } } let global_block=!request.policy_allowed||!request.protected_closure||!request.signed_approval||!request.raw_data_local||!request.aggregate_only||!request.adversarial_events.is_empty(); if global_block { denied.extend(action_order.iter().cloned()); admitted.clear();approval.clear();omission.insert("request:governance-or-adversarial-gate-blocked".into()); } uncertainty.extend(request.adversarial_events.iter().map(|event|format!("adversarial:{event}"))); let ao=admitted.iter().cloned().collect::<Vec<_>>();let po=approval.iter().cloned().collect::<Vec<_>>();let do_=denied.iter().cloned().collect::<Vec<_>>();let disposition=if global_block||ao.is_empty()&&po.is_empty(){"blocked"}else if !po.is_empty()||!do_.is_empty()||!revoked.is_empty()||!scope_mismatch.is_empty()||!over_budget.is_empty(){"unresolved"}else{"qualified"};if disposition!="qualified"{omission.insert("request:policy-closure-not-ready".into());} let mut payload=json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"scope":request.scope,"max_autonomy_tier":request.max_autonomy_tier,"disposition":disposition,"action_order":action_order,"admitted_order":ao,"approval_required_order":po,"denied_order":do_,"revoked_order":revoked.iter().cloned().collect::<Vec<_>>(),"over_budget_order":over_budget.iter().cloned().collect::<Vec<_>>(),"scope_mismatch_order":scope_mismatch.iter().cloned().collect::<Vec<_>>(),"missing_authority_order":missing_authority.iter().cloned().collect::<Vec<_>>(),"omission_order":omission.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});let receipt_digest=ContentHash::of_value(&payload).map_err(|error|PolicyAutonomyWorkbenchError::Output(error.to_string()))?;payload["receipt_digest"]=json!(receipt_digest);payload["artifact"]=json!({"artifact_id":format!("policy-receipt-5:{}",request.request_id),"content_type":CONTENT_TYPE,"content_hash":receipt_digest,"semantic_loss":omission.iter().cloned().collect::<Vec<_>>(),"provenance_digests":provenance.iter().cloned().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY});payload["effect_receipts"]=json!(if disposition=="qualified"{vec![format!("view:authorized-research-state:{}",request.request_id)]}else{vec!["block:unsafe-release".to_string()]});let output:PolicyReceipt5=serde_json::from_value(payload).map_err(|error|PolicyAutonomyWorkbenchError::Output(error.to_string()))?;output.validate()?;Ok(output) }

#[cfg(test)] mod tests { use super::*; fn h(value:&str)->ContentHash{ContentHash::of_bytes(value.as_bytes())} fn action(id:&str)->ActionAndAuthority4{ActionAndAuthority4{action_id:id.into(),actor:"researcher".into(),requested_effect:"view:authorized-research-state".into(),scope:"study:local".into(),autonomy_tier:"a1".into(),resource_budget:100,resource_cost:1,expires_at:1000,revoked:false,evidence_state:ActionEvidenceState::Proven,provenance_digest:h("prov"),replay_identity:h("replay"),local:true,aggregate_only:true,omission_order:vec![],negative_result:false}} fn request()->ActionAndAuthorityRequest4{ActionAndAuthorityRequest4{schema_version:INPUT_SCHEMA.into(),request_id:"policy:req".into(),scope:"study:local".into(),allowed_effect_order:vec!["view:authorized-research-state".into()],max_autonomy_tier:"a1".into(),now_epoch:1,actions:vec![action("b"),action("a")],replay_identity:h("replay"),policy_allowed:true,protected_closure:true,signed_approval:true,raw_data_local:true,aggregate_only:true,adversarial_events:vec![],boundary:PRECLINICAL_BOUNDARY.into()}} #[test]fn qualified(){assert_eq!(operate_policy_autonomy(&request()).unwrap().disposition,"qualified")} #[test]fn revocation_is_visible(){let mut q=request();q.actions[0].revoked=true;assert!(!operate_policy_autonomy(&q).unwrap().revoked_order.is_empty())} #[test]fn policy_blocks(){let mut q=request();q.policy_allowed=false;assert_eq!(operate_policy_autonomy(&q).unwrap().disposition,"blocked")} }
