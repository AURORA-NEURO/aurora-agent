//! World P32 multimodal multi-study contract-model causal-integrity feature F06.
use super::causal_integrity_support::{qualify,manifest,CausalIntegrityCard7,CausalIntegrityRequest4};
const FEATURE_ID:&str="AFA-world-P32-F06";const CONTRACT_VERSION:&str="world-multimodal-causal-integrity-contract-model/1.0";
pub fn world_multimodal_causal_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract-model")}
pub fn qualify_world_multimodal_causal_integrity_contract_model(request:&CausalIntegrityRequest4)->Result<CausalIntegrityCard7,super::causal_integrity_support::CausalIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract-model")}

