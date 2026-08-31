//! Obligation P32 local single-study contract-model closure-gate feature F02.
use super::closure_gate_support::{certify,manifest,ClosureGateCard7,ClosureGateRequest4};
const FEATURE_ID:&str="AFA-obligation-P32-F02";const CONTRACT_VERSION:&str="obligation-local-closure-gate-contract-model/1.0";
pub fn obligation_local_closure_gate_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn certify_obligation_local_closure_gate_contract_model(request:&ClosureGateRequest4)->Result<ClosureGateCard7,super::closure_gate_support::ClosureGateError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
