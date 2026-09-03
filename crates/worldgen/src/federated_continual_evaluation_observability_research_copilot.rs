//! Worldgen P23 federated_continual evaluation/observability research copilot.
use super::evaluation_observability_support::{self,EvaluationRequest4,EvaluationCard8};
pub const FEATURE_ID:&str="AFA-worldgen-P23-F12"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-evaluation-observability-copilot/1.0";
pub fn worldgen_federated_continual_evaluation_observability_research_copilot_manifest()->serde_json::Value{evaluation_observability_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","copilot")}
pub fn run_worldgen_federated_continual_evaluation_observability_research_copilot(request:&EvaluationRequest4)->Result<EvaluationCard8,evaluation_observability_support::EvaluationObservabilityError>{evaluation_observability_support::evaluate(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","copilot")}
