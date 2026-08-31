//! Worldgen P13 F03 statistical, causal, and ML inference.
use super::statistical_causal_ml_support::{self,AnalysisQuestion3,QualifiedAnalysisResult1};
pub const FEATURE_ID:&str="AFA-worldgen-P13-F03"; pub const CONTRACT_VERSION:&str="worldgen-throughput-statistical-causal-ml/1.0";
pub fn worldgen_throughput_statistical_causal_ml_inference_manifest()->serde_json::Value{statistical_causal_ml_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn qualify_worldgen_throughput_statistical_causal_ml_analysis(request:&AnalysisQuestion3)->Result<QualifiedAnalysisResult1,statistical_causal_ml_support::StatisticalCausalMlError>{statistical_causal_ml_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use statistical_causal_ml_support::{AnalysisCandidate,AnalysisEvidenceState,StatisticalCausalMlError};

