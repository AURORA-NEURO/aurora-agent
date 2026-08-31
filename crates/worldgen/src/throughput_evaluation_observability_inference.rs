//! Worldgen P23 throughput evaluation/observability inference.
use super::evaluation_observability_support::{self,EvaluationRequest4,EvaluationCard8};
pub const FEATURE_ID:&str="AFA-worldgen-P23-F03"; pub const CONTRACT_VERSION:&str="worldgen-throughput-evaluation-observability/1.0";
pub fn worldgen_throughput_evaluation_observability_inference_manifest()->serde_json::Value{evaluation_observability_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","inference")}
pub fn evaluate_worldgen_throughput_evaluation_observability(request:&EvaluationRequest4)->Result<EvaluationCard8,evaluation_observability_support::EvaluationObservabilityError>{evaluation_observability_support::evaluate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","inference")}
