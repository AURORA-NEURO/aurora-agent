//! Baseline P32 local single-study inference counterfactual-integrity feature F01.
use super::counterfactual_integrity_support::{qualify,manifest,CounterfactualIntegrityCard7,CounterfactualIntegrityRequest4};
const FEATURE_ID:&str="AFA-baseline-P32-F01";const CONTRACT_VERSION:&str="baseline-local-counterfactual-integrity-inference/1.0";
pub fn baseline_local_counterfactual_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_baseline_local_counterfactual_integrity_inference(request:&CounterfactualIntegrityRequest4)->Result<CounterfactualIntegrityCard7,super::counterfactual_integrity_support::CounterfactualIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
