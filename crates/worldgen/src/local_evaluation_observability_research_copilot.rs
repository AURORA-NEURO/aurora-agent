//! Worldgen P23 local evaluation/observability research copilot.
use super::evaluation_observability_support::{self,EvaluationRequest4,EvaluationCard8};
pub const FEATURE_ID:&str="AFA-worldgen-P23-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-evaluation-observability-copilot/1.0";
pub fn worldgen_local_evaluation_observability_research_copilot_manifest()->serde_json::Value{evaluation_observability_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","copilot")}
pub fn run_worldgen_local_evaluation_observability_research_copilot(request:&EvaluationRequest4)->Result<EvaluationCard8,evaluation_observability_support::EvaluationObservabilityError>{evaluation_observability_support::evaluate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","copilot")}
