//! PRISM P32 local single-study research-copilot evaluation-integrity feature F03.
use super::evaluation_integrity_support::{evaluate,manifest,EvaluationIntegrityCard7,EvaluationIntegrityRequest4};
const FEATURE_ID:&str="AFA-prism-P32-F03";const CONTRACT_VERSION:&str="prism-local-evaluation-integrity-research-copilot/1.0";
pub fn prism_local_evaluation_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
pub fn evaluate_prism_local_evaluation_integrity_research_copilot(request:&EvaluationIntegrityRequest4)->Result<EvaluationIntegrityCard7,super::evaluation_integrity_support::EvaluationIntegrityError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
