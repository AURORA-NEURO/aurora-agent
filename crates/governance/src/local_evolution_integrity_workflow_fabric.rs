//! Governance P32 local single-study workflow_fabric evolution-integrity feature F04.
use super::evolution_integrity_support::{qualify,manifest,EvolutionIntegrityCard7,EvolutionIntegrityRequest4,EvolutionIntegrityError};
const FEATURE_ID:&str="AFA-governance-P32-F04";const CONTRACT_VERSION:&str="governance-local-evolution-integrity-workflow_fabric/1.0";
pub fn governance_local_evolution_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow_fabric")}
pub fn qualify_governance_local_evolution_integrity_workflow_fabric(request:&EvolutionIntegrityRequest4)->Result<EvolutionIntegrityCard7,EvolutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow_fabric")}
