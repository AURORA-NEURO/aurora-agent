//! Cross-site security/federation contract negotiation for Worldgen P20.
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.worldgen.security-federation-contract-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFederationContractRequest {
    pub request_id: String,
    pub consumer: String,
    pub producer: String,
    pub namespace: String,
    pub semantic_profile: String,
    pub negotiated_version: String,
    pub field_order: Vec<String>,
    pub retained_field_order: Vec<String>,
    pub missing_field_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub federation_authorized: bool,
    pub key_active: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFederationContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub producer: String,
    pub namespace: String,
    pub semantic_profile: String,
    pub negotiated_version: String,
    pub compatibility: String,
    pub disposition: String,
    pub field_order: Vec<String>,
    pub retained_field_order: Vec<String>,
    pub missing_field_order: Vec<String>,
    pub redacted_field_order: Vec<String>,
    pub security_issue_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub contract_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecurityFederationContractError {
    #[error("invalid security/federation contract request: {0}")]
    Invalid(String),
    #[error("invalid security/federation contract receipt: {0}")]
    Receipt(String),
    #[error("security/federation contract artifact failed: {0}")]
    Artifact(String),
}
fn hash(v: &ContentHash) -> bool { v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit()) }
fn ordered(v: &[String]) -> bool { v.windows(2).all(|w| w[0] < w[1]) }

impl SecurityFederationContractReceipt {
    pub fn validate(&self) -> Result<(), SecurityFederationContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|v| v.as_str()) != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|v| v.as_str()) != Some(CONTENT_TYPE)
            || !self.raw_data_local || !self.aggregate_only || self.field_order.is_empty()
            || !hash(&self.replay_identity) || !hash(&self.contract_digest)
            || self.artifact.get("content_hash").and_then(|v| v.as_str()) != Some(self.contract_digest.as_str())
        { return Err(SecurityFederationContractError::Receipt("security contract identity, locality, or digest is incomplete".into())); }
        for values in [&self.field_order, &self.retained_field_order, &self.missing_field_order, &self.redacted_field_order, &self.security_issue_order, &self.effect_receipts] {
            if !ordered(values) { return Err(SecurityFederationContractError::Receipt("security contract vectors are not canonical".into())); }
        }
        let fields = self.field_order.iter().cloned().collect::<BTreeSet<_>>();
        let represented = self.retained_field_order.iter().chain(&self.missing_field_order).chain(&self.redacted_field_order).cloned().collect::<BTreeSet<_>>();
        if fields != represented { return Err(SecurityFederationContractError::Receipt("security contract fields do not partition".into())); }
        Ok(())
    }
}

pub fn manifest(feature_id: &str, version: &str, scale: &str) -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["security steward","federation operator","developer"],"behavior":format!("negotiate signed security/federation fields for {scale}"),"value":"makes schema compatibility, redaction, key state, and locality explicit before exchange","input_schema":"SecurityFederationContractRequest1@1","output_schema":"SecurityFederationContractReceipt1@1","effects":["none:security-contract-validation","block:unsafe-export"],"permissions":["negotiate:federation-contract"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

pub fn negotiate(r: &SecurityFederationContractRequest, feature_id: &str, version: &str, _scale: &str, _federated: bool) -> Result<SecurityFederationContractReceipt, SecurityFederationContractError> {
    if r.request_id.trim().is_empty() || r.consumer.trim().is_empty() || r.producer.trim().is_empty() || r.field_order.is_empty() || r.boundary != PRECLINICAL_BOUNDARY || !r.raw_data_local || !r.aggregate_only || !hash(&r.replay_identity) { return Err(SecurityFederationContractError::Invalid("security contract request is invalid".into())); }
    let fields = r.field_order.iter().cloned().collect::<BTreeSet<_>>();
    let retained = r.retained_field_order.iter().filter(|x| fields.contains(*x)).cloned().collect::<BTreeSet<_>>();
    let missing = fields.difference(&retained).cloned().collect::<BTreeSet<_>>();
    let redacted = r.missing_field_order.iter().filter(|x| fields.contains(*x)).cloned().collect::<BTreeSet<_>>();
    let mut issues: BTreeSet<String> = BTreeSet::new();
    if !r.policy_allow { issues.insert("policy-denied".into()); }
    if !r.protected_closure { issues.insert("protected-closure-incomplete".into()); }
    if !r.federation_authorized { issues.insert("federation-authorization-missing".into()); }
    if !r.key_active { issues.insert("signing-key-inactive".into()); }
    let disposition = if !issues.is_empty() { "blocked" } else if retained.is_empty() { "unresolved" } else if missing.is_empty() && redacted.is_empty() { "compatible" } else { "partial" };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":version,"feature_id":feature_id,"request_id":r.request_id,"consumer":r.consumer,"producer":r.producer,"namespace":r.namespace,"semantic_profile":r.semantic_profile,"negotiated_version":r.negotiated_version,"compatibility":if disposition=="compatible"{"compatible"}else{"redacted-migration"},"disposition":disposition,"field_order":fields,"retained_field_order":retained,"missing_field_order":missing,"redacted_field_order":redacted,"security_issue_order":issues,"replay_identity":r.replay_identity,"effect_receipts":["none:security-contract-validation"],"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let d = ContentHash::of_value(&payload).map_err(|e| SecurityFederationContractError::Artifact(e.to_string()))?;
    let strings = |k: &str| payload[k].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let out = SecurityFederationContractReceipt { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:version.into(), feature_id:feature_id.into(), request_id:r.request_id.clone(), consumer:r.consumer.clone(), producer:r.producer.clone(), namespace:r.namespace.clone(), semantic_profile:r.semantic_profile.clone(), negotiated_version:r.negotiated_version.clone(), compatibility:if disposition=="compatible"{"compatible"}else{"redacted-migration"}.into(), disposition:disposition.into(), field_order:strings("field_order"), retained_field_order:strings("retained_field_order"), missing_field_order:strings("missing_field_order"), redacted_field_order:strings("redacted_field_order"), security_issue_order:strings("security_issue_order"), replay_identity:r.replay_identity.clone(), contract_digest:d.clone(), effect_receipts:vec!["none:security-contract-validation".into()], artifact:json!({"content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}), raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into() };
    out.validate()?; Ok(out)
}
