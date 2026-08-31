//! World P32 local single-study workflow-fabric causal-integrity feature F04.
use super::causal_integrity_support::{qualify,manifest,CausalIntegrityCard7,CausalIntegrityRequest4};
const FEATURE_ID:&str="AFA-world-P32-F04";const CONTRACT_VERSION:&str="world-local-causal-integrity-workflow-fabric/1.0";
pub fn world_local_causal_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn qualify_world_local_causal_integrity_workflow_fabric(request:&CausalIntegrityRequest4)->Result<CausalIntegrityCard7,super::causal_integrity_support::CausalIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}

