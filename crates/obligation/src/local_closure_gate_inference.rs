//! Obligation P32 local single-study inference closure-gate feature F01.
use super::closure_gate_support::{certify,manifest,ClosureGateCard7,ClosureGateRequest4};
const FEATURE_ID:&str="AFA-obligation-P32-F01";const CONTRACT_VERSION:&str="obligation-local-closure-gate-inference/1.0";
pub fn obligation_local_closure_gate_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn certify_obligation_local_closure_gate_inference(request:&ClosureGateRequest4)->Result<ClosureGateCard7,super::closure_gate_support::ClosureGateError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
