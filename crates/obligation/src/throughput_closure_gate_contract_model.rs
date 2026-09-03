//! Obligation P32 prospective high-throughput contract-model closure-gate feature F10.
use super::closure_gate_support::{certify,manifest,ClosureGateCard7,ClosureGateRequest4};
const FEATURE_ID:&str="AFA-obligation-P32-F10";const CONTRACT_VERSION:&str="obligation-throughput-closure-gate-contract-model/1.0";
pub fn obligation_throughput_closure_gate_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract-model")}
pub fn certify_obligation_throughput_closure_gate_contract_model(request:&ClosureGateRequest4)->Result<ClosureGateCard7,super::closure_gate_support::ClosureGateError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract-model")}
