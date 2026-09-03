//! World P32 local single-study inference causal-integrity feature F01.
use super::causal_integrity_support::{qualify,manifest,CausalIntegrityCard7,CausalIntegrityRequest4};
const FEATURE_ID:&str="AFA-world-P32-F01";const CONTRACT_VERSION:&str="world-local-causal-integrity-inference/1.0";
pub fn world_local_causal_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_world_local_causal_integrity_inference(request:&CausalIntegrityRequest4)->Result<CausalIntegrityCard7,super::causal_integrity_support::CausalIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}

