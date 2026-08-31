//! Obligation P32 local single-study workflow-fabric closure-gate feature F04.
use super::closure_gate_support::{certify,manifest,ClosureGateCard7,ClosureGateRequest4};
const FEATURE_ID:&str="AFA-obligation-P32-F04";const CONTRACT_VERSION:&str="obligation-local-closure-gate-workflow-fabric/1.0";
pub fn obligation_local_closure_gate_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn certify_obligation_local_closure_gate_workflow_fabric(request:&ClosureGateRequest4)->Result<ClosureGateCard7,super::closure_gate_support::ClosureGateError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
