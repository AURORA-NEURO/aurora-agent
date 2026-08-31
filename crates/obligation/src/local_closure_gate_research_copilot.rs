//! Obligation P32 local single-study research-copilot closure-gate feature F03.
use super::closure_gate_support::{certify,manifest,ClosureGateCard7,ClosureGateRequest4};
const FEATURE_ID:&str="AFA-obligation-P32-F03";const CONTRACT_VERSION:&str="obligation-local-closure-gate-research-copilot/1.0";
pub fn obligation_local_closure_gate_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
pub fn certify_obligation_local_closure_gate_research_copilot(request:&ClosureGateRequest4)->Result<ClosureGateCard7,super::closure_gate_support::ClosureGateError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
