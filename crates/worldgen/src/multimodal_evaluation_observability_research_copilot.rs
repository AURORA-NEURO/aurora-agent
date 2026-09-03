//! Worldgen P23 multimodal evaluation/observability research copilot.
use super::evaluation_observability_support::{self,EvaluationRequest4,EvaluationCard8};
pub const FEATURE_ID:&str="AFA-worldgen-P23-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-evaluation-observability-copilot/1.0";
pub fn worldgen_multimodal_evaluation_observability_research_copilot_manifest()->serde_json::Value{evaluation_observability_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","copilot")}
pub fn run_worldgen_multimodal_evaluation_observability_research_copilot(request:&EvaluationRequest4)->Result<EvaluationCard8,evaluation_observability_support::EvaluationObservabilityError>{evaluation_observability_support::evaluate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","copilot")}
