//! Worldgen P23 federated_continual evaluation/observability inference.
use super::evaluation_observability_support::{self,EvaluationRequest4,EvaluationCard8};
pub const FEATURE_ID:&str="AFA-worldgen-P23-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-evaluation-observability/1.0";
pub fn worldgen_federated_continual_evaluation_observability_inference_manifest()->serde_json::Value{evaluation_observability_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","inference")}
pub fn evaluate_worldgen_federated_continual_evaluation_observability(request:&EvaluationRequest4)->Result<EvaluationCard8,evaluation_observability_support::EvaluationObservabilityError>{evaluation_observability_support::evaluate(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","inference")}
