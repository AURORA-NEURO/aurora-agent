//! Mutation P32 multimodal contract_model evolution-integrity feature.
use super::evolution_integrity_support::{manifest,qualify,EvolutionCard7,EvolutionIntegrityError,EvolutionRequest4};
pub const FEATURE_ID:&str="AFA-mutation-P32-F06";pub const CONTRACT_VERSION:&str="mutation-multimodal_evolution_integrity_contract_model/1.0";
pub fn multimodal_evolution_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","contract_model")}
pub fn qualify_multimodal_evolution_integrity_contract_model(request:&EvolutionRequest4)->Result<EvolutionCard7,EvolutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","contract_model")}
