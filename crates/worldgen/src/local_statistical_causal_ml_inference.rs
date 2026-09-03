//! Worldgen P13 F01 statistical, causal, and ML inference.
use super::statistical_causal_ml_support::{self,AnalysisQuestion3,QualifiedAnalysisResult1};
pub const FEATURE_ID:&str="AFA-worldgen-P13-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-statistical-causal-ml/1.0";
pub fn worldgen_local_statistical_causal_ml_inference_manifest()->serde_json::Value{statistical_causal_ml_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn qualify_worldgen_local_statistical_causal_ml_analysis(request:&AnalysisQuestion3)->Result<QualifiedAnalysisResult1,statistical_causal_ml_support::StatisticalCausalMlError>{statistical_causal_ml_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use statistical_causal_ml_support::{AnalysisCandidate,AnalysisEvidenceState,StatisticalCausalMlError};

