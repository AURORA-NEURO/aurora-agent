//! Baseline P32 local single-study workflow-fabric counterfactual-integrity feature F04.
use super::counterfactual_integrity_support::{qualify,manifest,CounterfactualIntegrityCard7,CounterfactualIntegrityRequest4};
const FEATURE_ID:&str="AFA-baseline-P32-F04";const CONTRACT_VERSION:&str="baseline-local-counterfactual-integrity-workflow_fabric/1.0";
pub fn baseline_local_counterfactual_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn qualify_baseline_local_counterfactual_integrity_workflow_fabric(request:&CounterfactualIntegrityRequest4)->Result<CounterfactualIntegrityCard7,super::counterfactual_integrity_support::CounterfactualIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
