//! Obligation P32 prospective high-throughput workflow-fabric closure-gate feature F12.
use super::closure_gate_support::{certify,manifest,ClosureGateCard7,ClosureGateRequest4};
const FEATURE_ID:&str="AFA-obligation-P32-F12";const CONTRACT_VERSION:&str="obligation-throughput-closure-gate-workflow-fabric/1.0";
pub fn obligation_throughput_closure_gate_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","workflow-fabric")}
pub fn certify_obligation_throughput_closure_gate_workflow_fabric(request:&ClosureGateRequest4)->Result<ClosureGateCard7,super::closure_gate_support::ClosureGateError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","workflow-fabric")}
