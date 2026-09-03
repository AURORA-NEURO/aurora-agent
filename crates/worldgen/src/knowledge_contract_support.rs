//! Versioned knowledge-representation contract negotiation for Worldgen P04 F05-F08.
use std::collections::BTreeSet;
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.knowledge-contract-receipt+json";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeContractRequest {
    pub request_id:String, pub consumer:String, pub producer:String, pub namespace:String,
    pub semantic_profile:String, pub negotiated_version:String, pub field_order:Vec<String>,
    pub retained_field_order:Vec<String>, pub missing_field_order:Vec<String>, pub replay_identity:ContentHash,
    pub policy_allow:bool, pub protected_closure:bool, pub raw_data_local:bool, pub aggregate_only:bool, pub boundary:String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeContractReceipt {
    pub schema_version:String, pub contract_version:String, pub feature_id:String, pub request_id:String,
    pub consumer:String, pub producer:String, pub namespace:String, pub semantic_profile:String,
    pub negotiated_version:String, pub compatibility:String, pub disposition:String,
    pub field_order:Vec<String>, pub retained_field_order:Vec<String>, pub missing_field_order:Vec<String>,
    pub omitted_field_order:Vec<String>, pub semantic_loss_order:Vec<String>, pub replay_identity:ContentHash,
    pub contract_digest:ContentHash, pub effect_receipts:Vec<String>, pub artifact:serde_json::Value,
    pub raw_data_local:bool, pub aggregate_only:bool, pub boundary:String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KnowledgeContractError {
    #[error("invalid knowledge contract request: {0}")] Invalid(String),
    #[error("invalid knowledge contract receipt: {0}")] Receipt(String),
    #[error("knowledge contract artifact failed: {0}")] Artifact(String),
}
fn digest(value:&ContentHash)->bool{value.as_str().len()==64&&value.as_str().bytes().all(|byte|byte.is_ascii_hexdigit())}
fn ordered(values:&[String])->bool{values.windows(2).all(|pair|pair[0]<pair[1])}
fn sorted(values:&[String])->Vec<String>{let mut out=values.to_vec();out.sort();out.dedup();out}
impl KnowledgeContractReceipt {
    pub fn validate(&self)->Result<(),KnowledgeContractError>{
        if self.schema_version!=RESEARCH_CONTRACT_SCHEMA_VERSION||self.boundary!=PRECLINICAL_BOUNDARY||self.artifact.get("boundary").and_then(|v|v.as_str())!=Some(PRECLINICAL_BOUNDARY)||self.artifact.get("content_type").and_then(|v|v.as_str())!=Some(CONTENT_TYPE)||!self.raw_data_local||!self.aggregate_only||self.request_id.trim().is_empty()||self.consumer.trim().is_empty()||self.producer.trim().is_empty()||self.namespace.trim().is_empty()||self.semantic_profile.trim().is_empty()||self.negotiated_version.trim().is_empty()||self.field_order.is_empty()||self.effect_receipts!=vec!["none:knowledge-contract-validation".to_string()]||![&self.replay_identity,&self.contract_digest].into_iter().all(digest){return Err(KnowledgeContractError::Receipt("knowledge contract identity, fields, locality, digests, or effects are incomplete".into()));}
        for values in [&self.field_order,&self.retained_field_order,&self.missing_field_order,&self.omitted_field_order,&self.semantic_loss_order]{if !ordered(values){return Err(KnowledgeContractError::Receipt("knowledge contract ordering is not canonical".into()));}}
        let fields=self.field_order.iter().cloned().collect::<BTreeSet<_>>();let parts=self.retained_field_order.iter().chain(&self.missing_field_order).chain(&self.omitted_field_order).cloned().collect::<Vec<_>>();
        if fields.len()!=self.field_order.len()||parts.len()!=fields.len()||parts.iter().cloned().collect::<BTreeSet<_>>()!=fields{return Err(KnowledgeContractError::Receipt("knowledge contract fields do not partition".into()));}
        if self.artifact.get("content_hash").and_then(|v|v.as_str())!=Some(self.contract_digest.as_str()){return Err(KnowledgeContractError::Receipt("knowledge contract artifact digest is inconsistent".into()));}
        Ok(())
    }
}
pub fn manifest(feature_id:&str,version:&str,input_schema:&str,scale:&str,autonomy:&str)->serde_json::Value{json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["schema steward","knowledge engineer","downstream graph consumer"],"behavior":format!("negotiate a versioned typed knowledge contract for {scale}"),"value":"makes knowledge schema compatibility and semantic loss explicit before graph reuse","input_schema":input_schema,"output_schema":"KnowledgeContractReceipt1@1","effects":["none:knowledge-contract-validation","block:unsafe-release"],"permissions":["negotiate:knowledge-contract"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":PRECLINICAL_BOUNDARY,"contract_version":version})}
pub fn negotiate(request:&KnowledgeContractRequest,feature_id:&str,version:&str,scale:&str,require_federation:bool)->Result<KnowledgeContractReceipt,KnowledgeContractError>{
    if request.request_id.trim().is_empty()||request.consumer.trim().is_empty()||request.producer.trim().is_empty()||request.namespace.trim().is_empty()||request.semantic_profile.trim().is_empty()||request.negotiated_version.trim().is_empty()||request.field_order.is_empty()||request.field_order!=sorted(&request.field_order)||request.field_order.iter().collect::<BTreeSet<_>>().len()!=request.field_order.len()||request.boundary!=PRECLINICAL_BOUNDARY||!request.raw_data_local||!request.aggregate_only||!digest(&request.replay_identity){return Err(KnowledgeContractError::Invalid("knowledge contract identity, fields, locality, boundary, ordering, or replay is invalid".into()));}
    if require_federation&&!request.policy_allow{return Err(KnowledgeContractError::Invalid("knowledge contract federation policy is denied".into()));}
    let fields=request.field_order.iter().cloned().collect::<BTreeSet<_>>();let retained=request.retained_field_order.iter().filter(|field|fields.contains(*field)).cloned().collect::<BTreeSet<_>>();let missing=fields.difference(&retained).cloned().collect::<BTreeSet<_>>();let omitted=request.missing_field_order.iter().filter(|field|fields.contains(*field)).cloned().collect::<BTreeSet<_>>();let semantic_loss=missing.union(&omitted).cloned().collect::<BTreeSet<_>>();let compatible=missing.is_empty()&&omitted.is_empty()&&request.protected_closure;
    let disposition=if !request.policy_allow||!request.protected_closure||(require_federation&&!request.aggregate_only){"blocked"}else if compatible{"compatible"}else if retained.is_empty(){"unknown"}else{"partial"};let compatibility=if compatible{"compatible"}else if retained.is_empty(){"unknown"}else{"additive_migration"};let effects=vec!["none:knowledge-contract-validation".to_string()];
    let payload=json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":version,"feature_id":feature_id,"request_id":request.request_id,"consumer":request.consumer,"producer":request.producer,"namespace":request.namespace,"semantic_profile":request.semantic_profile,"negotiated_version":request.negotiated_version,"compatibility":compatibility,"disposition":disposition,"field_order":fields,"retained_field_order":retained,"missing_field_order":missing,"omitted_field_order":omitted,"semantic_loss_order":semantic_loss,"replay_identity":request.replay_identity,"effect_receipts":effects,"boundary":PRECLINICAL_BOUNDARY});
    let contract_digest=ContentHash::of_value(&payload).map_err(|e|KnowledgeContractError::Artifact(e.to_string()))?;let receipt=KnowledgeContractReceipt{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),contract_version:version.into(),feature_id:feature_id.into(),request_id:request.request_id.clone(),consumer:request.consumer.clone(),producer:request.producer.clone(),namespace:request.namespace.clone(),semantic_profile:request.semantic_profile.clone(),negotiated_version:request.negotiated_version.clone(),compatibility:compatibility.into(),disposition:disposition.into(),field_order:fields.iter().cloned().collect(),retained_field_order:retained.iter().cloned().collect(),missing_field_order:missing.iter().cloned().collect(),omitted_field_order:omitted.iter().cloned().collect(),semantic_loss_order:semantic_loss.iter().cloned().collect(),replay_identity:request.replay_identity.clone(),contract_digest:contract_digest.clone(),effect_receipts:effects,artifact:json!({"artifact_id":format!("worldgen-knowledge-contract:{}",request.request_id),"content_type":CONTENT_TYPE,"content_hash":contract_digest,"boundary":PRECLINICAL_BOUNDARY}),raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()};receipt.validate()?;Ok(receipt)
}
#[cfg(test)]mod tests{use super::*;fn hash(v:&str)->ContentHash{ContentHash::of_bytes(v.as_bytes())}fn request()->KnowledgeContractRequest{KnowledgeContractRequest{request_id:"contract:req".into(),consumer:"consumer:graph".into(),producer:"producer:context".into(),namespace:"preclinical".into(),semantic_profile:"kg-v1".into(),negotiated_version:"1.0".into(),field_order:vec!["evidence".into(),"provenance".into()],retained_field_order:vec!["evidence".into(),"provenance".into()],missing_field_order:Vec::new(),replay_identity:hash("replay"),policy_allow:true,protected_closure:true,raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()}}#[test]fn compatible_contract_is_typed(){let r=negotiate(&request(),"AFA-worldgen-P04-F05","worldgen-local-knowledge-contract/1.0","local single-study",false).unwrap();assert_eq!(r.disposition,"compatible")}#[test]fn semantic_loss_is_explicit(){let mut q=request();q.retained_field_order=vec!["evidence".into()];let r=negotiate(&q,"AFA-worldgen-P04-F06","worldgen-multimodal-knowledge-contract/1.0","multimodal multi-study",false).unwrap();assert_eq!(r.disposition,"partial");}}

