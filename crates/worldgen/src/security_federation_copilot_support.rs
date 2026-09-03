//! Approval-bounded security/federation copilot for Worldgen P20.
use super::security_federation_support::{self, SecurityFederationReceipt, SecurityFederationRequest};
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.security-federation-copilot-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFederationCopilotRequest {
    pub security_request: SecurityFederationRequest,
    pub action_order: Vec<String>,
    pub action_budget: u64,
    pub dry_run: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFederationCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub disposition: String,
    pub action_order: Vec<String>,
    pub admitted_action_order: Vec<String>,
    pub denied_action_order: Vec<String>,
    pub security_disposition: String,
    pub security_digest: ContentHash,
    pub copilot_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub threat_order: Vec<String>,
    pub revocation_order: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecurityFederationCopilotError {
    #[error("invalid security/federation copilot request: {0}")] Invalid(String),
    #[error("security admission failed: {0}")] Security(String),
    #[error("security copilot artifact failed: {0}")] Artifact(String),
    #[error("invalid security/federation copilot receipt: {0}")] Receipt(String),
}
fn hash(v:&ContentHash)->bool{v.as_str().len()==64&&v.as_str().bytes().all(|b|b.is_ascii_hexdigit())}
fn ordered(v:&[String])->bool{v.windows(2).all(|w|w[0]<w[1])}
impl SecurityFederationCopilotReceipt{pub fn validate(&self)->Result<(),SecurityFederationCopilotError>{if self.schema_version!=RESEARCH_CONTRACT_SCHEMA_VERSION||self.boundary!=PRECLINICAL_BOUNDARY||self.artifact.get("boundary").and_then(|v|v.as_str())!=Some(PRECLINICAL_BOUNDARY)||self.artifact.get("content_type").and_then(|v|v.as_str())!=Some(CONTENT_TYPE)||!self.raw_data_local||!self.aggregate_only||self.action_order.is_empty()||![&self.security_digest,&self.copilot_digest,&self.replay_identity].into_iter().all(hash)||self.artifact.get("content_hash").and_then(|v|v.as_str())!=Some(self.copilot_digest.as_str()){return Err(SecurityFederationCopilotError::Receipt("security copilot identity, locality, or digest is incomplete".into()))}for v in [&self.action_order,&self.admitted_action_order,&self.denied_action_order,&self.omissions,&self.threat_order,&self.revocation_order,&self.effect_receipts]{if !ordered(v){return Err(SecurityFederationCopilotError::Receipt("security copilot vectors are not canonical".into()))}}let ids=self.action_order.iter().cloned().collect::<BTreeSet<_>>();let parts=self.admitted_action_order.iter().chain(&self.denied_action_order).cloned().collect::<BTreeSet<_>>();if ids!=parts{return Err(SecurityFederationCopilotError::Receipt("security copilot actions do not partition".into()))}Ok(())}}
pub fn manifest(feature_id:&str,version:&str,scale:&str)->serde_json::Value{json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["security steward","federation operator","research program lead"],"behavior":format!("run approval-bounded security/federation actions for {scale}"),"value":"turns a security receipt into a replayable, approval-gated export decision without hidden effects","input_schema":"SecurityFederationCopilotRequest1@1","output_schema":"SecurityFederationCopilotReceipt1@1","effects":["invoke:bounded-federation-tool","block:unsafe-export"],"permissions":["invoke:declared-federation-tool"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})}
pub fn run(r:&SecurityFederationCopilotRequest,feature_id:&str,version:&str,_scale:&str,require_approval:bool,_federated:bool)->Result<SecurityFederationCopilotReceipt,SecurityFederationCopilotError>{if r.boundary!=PRECLINICAL_BOUNDARY||r.action_order.is_empty()||r.action_order!={let mut x=r.action_order.clone();x.sort();x.dedup();x}||r.action_budget==0||!r.security_request.raw_data_local||!r.security_request.aggregate_only{return Err(SecurityFederationCopilotError::Invalid("security copilot request is invalid".into()))}let security=security_federation_support::qualify(&r.security_request,feature_id,version).map_err(|e|SecurityFederationCopilotError::Security(e.to_string()))?;let mut omissions=security.omission_order.clone();let approved=!require_approval||r.signed_approval;if !approved{omissions.push("copilot:approval-missing".into())}if r.dry_run{omissions.push("copilot:dry-run-no-effect".into())}if !r.federation_approved{omissions.push("copilot:federation-approval-missing".into())}if r.action_order.len()as u64>r.action_budget{omissions.push("copilot:action-budget-exceeded".into())}omissions.sort();omissions.dedup();let safe=security.disposition=="admitted"&&approved&&r.federation_approved&&r.action_order.len()as u64<=r.action_budget;let disposition=if safe{"admitted"}else{"blocked"};let admitted=if safe{r.action_order.clone()}else{Vec::new()};let denied=if safe{Vec::new()}else{r.action_order.clone()};let effects=if safe{vec![format!("invoke:signed-aggregate-federation:{}",security.federation_digest)]}else{vec!["block:unsafe-export".into()]};let payload=json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":version,"feature_id":feature_id,"request_id":r.security_request.request_id,"disposition":disposition,"action_order":r.action_order,"admitted_action_order":admitted,"denied_action_order":denied,"security_disposition":security.disposition,"security_digest":security.federation_digest,"replay_identity":security.replay_identity,"omissions":omissions,"threat_order":security.threat_order,"revocation_order":security.revocation_order,"effect_receipts":effects,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});let d=ContentHash::of_value(&payload).map_err(|e|SecurityFederationCopilotError::Artifact(e.to_string()))?;let strings=|k:&str|payload[k].as_array().map(|a|a.iter().filter_map(|v|v.as_str().map(String::from)).collect()).unwrap_or_default();let out=SecurityFederationCopilotReceipt{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),contract_version:version.into(),feature_id:feature_id.into(),request_id:r.security_request.request_id.clone(),disposition:disposition.into(),action_order:r.action_order.clone(),admitted_action_order:admitted,denied_action_order:denied,security_disposition:security.disposition,security_digest:security.federation_digest,copilot_digest:d.clone(),replay_identity:security.replay_identity,omissions:strings("omissions"),threat_order:strings("threat_order"),revocation_order:strings("revocation_order"),effect_receipts:strings("effect_receipts"),artifact:json!({"content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}),raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()};out.validate()?;Ok(out)}
pub type SecurityFederationCopilotResult = SecurityFederationCopilotReceipt;
