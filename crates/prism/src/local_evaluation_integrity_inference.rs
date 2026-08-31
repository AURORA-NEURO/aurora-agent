//! PRISM P32 local single-study inference evaluation-integrity feature F01.
use super::evaluation_integrity_support::{evaluate,manifest,EvaluationIntegrityCard7,EvaluationIntegrityRequest4};
const FEATURE_ID:&str="AFA-prism-P32-F01";const CONTRACT_VERSION:&str="prism-local-evaluation-integrity-inference/1.0";
pub fn prism_local_evaluation_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn evaluate_prism_local_evaluation_integrity_inference(request:&EvaluationIntegrityRequest4)->Result<EvaluationIntegrityCard7,super::evaluation_integrity_support::EvaluationIntegrityError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
