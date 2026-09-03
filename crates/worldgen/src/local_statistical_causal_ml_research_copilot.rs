//! Worldgen P13 F09 statistical, causal, and ML research copilot.
use super::statistical_causal_ml_copilot_support::{self,AnalysisCopilotRequest,AnalysisCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P13-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-statistical-causal-ml-copilot/1.0";
pub fn worldgen_local_statistical_causal_ml_research_copilot_manifest()->serde_json::Value{statistical_causal_ml_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn run_worldgen_local_statistical_causal_ml_research_copilot(request:&AnalysisCopilotRequest)->Result<AnalysisCopilotReceipt,statistical_causal_ml_copilot_support::AnalysisCopilotError>{statistical_causal_ml_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use statistical_causal_ml_copilot_support::{AnalysisCopilotError,AnalysisCopilotRequest as WorldgenStatisticalCausalMlCopilotRequest,AnalysisCopilotReceipt as WorldgenStatisticalCausalMlCopilotReceipt};

