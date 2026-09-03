//! Mutation P32 throughput inference evolution-integrity feature.
use super::evolution_integrity_support::{manifest,qualify,EvolutionCard7,EvolutionIntegrityError,EvolutionRequest4};
pub const FEATURE_ID:&str="AFA-mutation-P32-F03";pub const CONTRACT_VERSION:&str="mutation-throughput_evolution_integrity_inference/1.0";
pub fn throughput_evolution_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
pub fn qualify_throughput_evolution_integrity_inference(request:&EvolutionRequest4)->Result<EvolutionCard7,EvolutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
