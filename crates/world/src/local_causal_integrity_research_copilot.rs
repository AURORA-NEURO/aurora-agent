//! World P32 local single-study research-copilot causal-integrity feature F03.
use super::causal_integrity_support::{qualify,manifest,CausalIntegrityCard7,CausalIntegrityRequest4};
const FEATURE_ID:&str="AFA-world-P32-F03";const CONTRACT_VERSION:&str="world-local-causal-integrity-research-copilot/1.0";
pub fn world_local_causal_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
pub fn qualify_world_local_causal_integrity_research_copilot(request:&CausalIntegrityRequest4)->Result<CausalIntegrityCard7,super::causal_integrity_support::CausalIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}

