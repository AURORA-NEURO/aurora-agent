//! Multimodal multi-study protocol-simulation research workbench
//! (`AFA-packs-P10-F18`).
//!
//! This packs-owned surface reuses the deterministic protocol state-machine kernel from the IDS
//! crate while publishing a packs-specific contract, artifact type, and researcher-facing gate.
//! It never runs protocols, contacts instruments, moves raw data, or makes clinical decisions.

use bioprism_ids::{simulate_protocol_workbench, ContentHash, ProtocolWorkbenchRequest5};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-packs-P10-F18";
pub const CONTRACT_VERSION: &str = "packs-multimodal-multi-study-protocol-simulation-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ProtocolWorkbenchRequest5@1";
pub const OUTPUT_SCHEMA: &str = "ProtocolWorkbenchReport9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.packs-protocol-workbench-report-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PacksProtocolWorkbenchReport9(pub Value);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PacksProtocolWorkbenchError {
    #[error("invalid packs protocol workbench request or receipt: {0}")]
    Invalid(String),
    #[error("packs protocol workbench artifact failed: {0}")]
    Artifact(String),
}

fn digest(v: &Value) -> bool { v.as_str().map(|s| s.len() == 64).unwrap_or(false) }
fn ordered(v: &Value) -> bool { v.as_array().map(|a| a.windows(2).all(|w| w[0].as_str() < w[1].as_str())).unwrap_or(false) }

impl PacksProtocolWorkbenchReport9 {
    pub fn validate(&self) -> Result<(), PacksProtocolWorkbenchError> {
        let v = &self.0;
        let artifact = v.get("artifact").ok_or_else(|| PacksProtocolWorkbenchError::Invalid("artifact is missing".into()))?;
        for key in ["request_id","federation_id","protocol_id","requester","purpose","semantic_profile"] { if v.get(key).and_then(Value::as_str).map(str::trim).unwrap_or("").is_empty() { return Err(PacksProtocolWorkbenchError::Invalid(format!("{key} is missing"))); } }
        if v.get("schema_version").and_then(Value::as_str) != Some("aurora-research-contract/1.0") || v.get("contract_version").and_then(Value::as_str) != Some(CONTRACT_VERSION) || v.get("feature_id").and_then(Value::as_str) != Some(FEATURE_ID) || v.get("boundary").and_then(Value::as_str) != Some(PRECLINICAL_BOUNDARY) || v.get("raw_data_local") != Some(&Value::Bool(true)) || v.get("aggregate_only") != Some(&Value::Bool(true)) { return Err(PacksProtocolWorkbenchError::Invalid("identity, boundary, or locality is invalid".into())); }
        for key in ["stage_order","qualified_stage_order","unresolved_stage_order","blocked_stage_order","scenario_order","passed_scenario_order","failed_scenario_order","unknown_scenario_order","negative_scenario_order","peer_order","qualified_peer_order","missing_peer_order","batch_order","capacity_order","omission_order","uncertainty_order","negative_evidence_order","recovery_order","effect_receipts"] { if !ordered(v.get(key).unwrap_or(&Value::Null)) { return Err(PacksProtocolWorkbenchError::Invalid(format!("{key} ordering is not canonical"))); } }
        if !digest(v.get("replay_identity").unwrap_or(&Value::Null)) || !digest(v.get("simulation_digest").unwrap_or(&Value::Null)) || artifact.get("content_type").and_then(Value::as_str) != Some(CONTENT_TYPE) || artifact.get("content_hash") != v.get("simulation_digest") || artifact.get("boundary").and_then(Value::as_str) != Some(PRECLINICAL_BOUNDARY) { return Err(PacksProtocolWorkbenchError::Artifact("protocol digest or artifact metadata is inconsistent".into())); }
        let effects = v.get("effect_receipts").and_then(Value::as_array).ok_or_else(|| PacksProtocolWorkbenchError::Invalid("effects are missing".into()))?; if effects.is_empty() || effects.iter().any(|e| e.as_str().map(|s| s != "block:unsafe-release" && !s.starts_with("view:packs-protocol-workbench:")).unwrap_or(true)) { return Err(PacksProtocolWorkbenchError::Invalid("effect is outside packs workbench gate".into())); }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, PacksProtocolWorkbenchError> { self.validate()?; ContentHash::of_value(&self.0).map_err(|e| PacksProtocolWorkbenchError::Artifact(e.to_string())) }
}

pub fn packs_protocol_workbench_manifest() -> Value { json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"packs","consumers":["protocol scientist","preclinical workbench operator","benchmark curator"],"behavior":"simulate multimodal multi-study protocol state machines and fault scenarios through a deterministic researcher workbench","value":"exposes protocol capacity, recovery, evidence, peer, and release gates before laboratory integration","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["view:packs-protocol-workbench"],"permissions":["read:local-protocol-manifests"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}) }

pub fn simulate_packs_protocol_workbench(request: &ProtocolWorkbenchRequest5) -> Result<PacksProtocolWorkbenchReport9, PacksProtocolWorkbenchError> {
    let old = simulate_protocol_workbench(request).map_err(|e| PacksProtocolWorkbenchError::Invalid(e.to_string()))?;
    let mut value = serde_json::to_value(old).map_err(|e| PacksProtocolWorkbenchError::Artifact(e.to_string()))?;
    value["contract_version"] = Value::String(CONTRACT_VERSION.into()); value["feature_id"] = Value::String(FEATURE_ID.into());
    value["artifact"]["content_type"] = Value::String(CONTENT_TYPE.into());
    let effects = if value.get("disposition").and_then(Value::as_str) == Some("qualified") { vec![format!("view:packs-protocol-workbench:{}", request.protocol_id)] } else { vec!["block:unsafe-release".into()] };
    value["effect_receipts"] = serde_json::to_value(effects).unwrap();
    let receipt = PacksProtocolWorkbenchReport9(value); receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests { use super::*; fn request()->ProtocolWorkbenchRequest5{ serde_json::from_value(json!({"request_id":"r","federation_id":"f","protocol_id":"p","requester":"scientist","purpose":"simulate","semantic_profile":"profile:v1","required_protocol_version":"1","stages":[{"stage_id":"s1","sequence":1,"input_schema":"in","output_schema":"out","required_capabilities":["compute"],"effect_class":"compute","estimated_units":1,"evidence_state":"supported","artifact_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","provenance_digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","deterministic":true,"local_only":true}],"scenarios":[{"scenario_id":"fault","fault_class":"timeout","affected_stages":["s1"],"observed_state":"supported","expected_recovery":"retry","budget_units":1,"replay_digest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","negative_result":false}],"peers":[{"peer_id":"peer","origin":"site","protocol_id":"p","semantic_profile":"profile:v1","checkpoint":1,"report_digest":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd","evidence_state":"supported","signed":true,"aggregate_only":true,"raw_data_local":true}],"checkpoint":1,"batch_size":1,"max_budget_units":4,"minimum_peer_quorum":1,"policy_allow":true,"protected_closure":true,"signed_approval":true,"federation_approved":true,"raw_data_local":true,"aggregate_only":true,"replay_identity":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee","boundary":PRECLINICAL_BOUNDARY})).unwrap() } #[test]fn manifest_is_a1(){assert_eq!(packs_protocol_workbench_manifest()["autonomy_tier"],"A1")} #[test]fn qualified(){assert!(simulate_packs_protocol_workbench(&request()).is_ok())} #[test]fn deterministic(){assert_eq!(simulate_packs_protocol_workbench(&request()).unwrap(),simulate_packs_protocol_workbench(&request()).unwrap())} }
