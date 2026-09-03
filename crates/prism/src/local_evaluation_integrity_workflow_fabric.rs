//! PRISM P32 local single-study workflow-fabric evaluation-integrity feature F04.
use super::evaluation_integrity_support::{evaluate,manifest,EvaluationIntegrityCard7,EvaluationIntegrityRequest4};
const FEATURE_ID:&str="AFA-prism-P32-F04";const CONTRACT_VERSION:&str="prism-local-evaluation-integrity-workflow-fabric/1.0";
pub fn prism_local_evaluation_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn evaluate_prism_local_evaluation_integrity_workflow_fabric(request:&EvaluationIntegrityRequest4)->Result<EvaluationIntegrityCard7,super::evaluation_integrity_support::EvaluationIntegrityError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
