//! PRISM P32 local single-study contract-model evaluation-integrity feature F02.
use super::evaluation_integrity_support::{evaluate,manifest,EvaluationIntegrityCard7,EvaluationIntegrityRequest4};
const FEATURE_ID:&str="AFA-prism-P32-F02";const CONTRACT_VERSION:&str="prism-local-evaluation-integrity-contract-model/1.0";
pub fn prism_local_evaluation_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn evaluate_prism_local_evaluation_integrity_contract_model(request:&EvaluationIntegrityRequest4)->Result<EvaluationIntegrityCard7,super::evaluation_integrity_support::EvaluationIntegrityError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
