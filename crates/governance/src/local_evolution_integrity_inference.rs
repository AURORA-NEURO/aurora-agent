//! Governance P32 local single-study inference evolution-integrity feature F01.
use super::evolution_integrity_support::{qualify,manifest,EvolutionIntegrityCard7,EvolutionIntegrityRequest4,EvolutionIntegrityError};
const FEATURE_ID:&str="AFA-governance-P32-F01";const CONTRACT_VERSION:&str="governance-local-evolution-integrity-inference/1.0";
pub fn governance_local_evolution_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_governance_local_evolution_integrity_inference(request:&EvolutionIntegrityRequest4)->Result<EvolutionIntegrityCard7,EvolutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
