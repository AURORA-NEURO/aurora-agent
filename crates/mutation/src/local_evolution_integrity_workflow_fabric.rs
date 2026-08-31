//! Mutation P32 local workflow_fabric evolution-integrity feature.
use super::evolution_integrity_support::{manifest,qualify,EvolutionCard7,EvolutionIntegrityError,EvolutionRequest4};
pub const FEATURE_ID:&str="AFA-mutation-P32-F13";pub const CONTRACT_VERSION:&str="mutation-local_evolution_integrity_workflow_fabric/1.0";
pub fn local_evolution_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","workflow_fabric")}
pub fn qualify_local_evolution_integrity_workflow_fabric(request:&EvolutionRequest4)->Result<EvolutionCard7,EvolutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","workflow_fabric")}
