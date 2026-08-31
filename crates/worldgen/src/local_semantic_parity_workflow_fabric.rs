//! Worldgen P28 local single-study workflow fabric feature F13.
use super::semantic_parity_support::{compare,manifest,SemanticParityCard7,SemanticParityRequest4};
const FEATURE_ID:&str="AFA-worldgen-P28-F13";const CONTRACT_VERSION:&str="worldgen-local-semantic-parity-workflow_fabric/1.0";
pub fn worldgen_local_semantic_parity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
pub fn compare_worldgen_local_semantic_parity_workflow(request:&SemanticParityRequest4)->Result<SemanticParityCard7,super::semantic_parity_support::SemanticParityError>{compare(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}

