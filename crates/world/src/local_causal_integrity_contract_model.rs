//! World P32 local single-study contract-model causal-integrity feature F02.
use super::causal_integrity_support::{qualify,manifest,CausalIntegrityCard7,CausalIntegrityRequest4};
const FEATURE_ID:&str="AFA-world-P32-F02";const CONTRACT_VERSION:&str="world-local-causal-integrity-contract-model/1.0";
pub fn world_local_causal_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn qualify_world_local_causal_integrity_contract_model(request:&CausalIntegrityRequest4)->Result<CausalIntegrityCard7,super::causal_integrity_support::CausalIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}

