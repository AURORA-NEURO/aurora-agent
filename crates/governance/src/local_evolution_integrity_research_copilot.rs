//! Governance P32 local single-study research_copilot evolution-integrity feature F03.
use super::evolution_integrity_support::{qualify,manifest,EvolutionIntegrityCard7,EvolutionIntegrityRequest4,EvolutionIntegrityError};
const FEATURE_ID:&str="AFA-governance-P32-F03";const CONTRACT_VERSION:&str="governance-local-evolution-integrity-research_copilot/1.0";
pub fn governance_local_evolution_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research_copilot")}
pub fn qualify_governance_local_evolution_integrity_research_copilot(request:&EvolutionIntegrityRequest4)->Result<EvolutionIntegrityCard7,EvolutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research_copilot")}
