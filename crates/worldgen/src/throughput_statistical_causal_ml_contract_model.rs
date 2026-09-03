//! Worldgen P13 F07 statistical, causal, and ML contract model.
use super::statistical_causal_ml_contract_support::{self,AnalysisContractRequest,AnalysisContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P13-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-statistical-causal-ml-contract/1.0";
pub fn worldgen_throughput_statistical_causal_ml_contract_model_manifest()->serde_json::Value{statistical_causal_ml_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn negotiate_worldgen_throughput_statistical_causal_ml_contract(request:&AnalysisContractRequest)->Result<AnalysisContractReceipt,statistical_causal_ml_contract_support::AnalysisContractError>{statistical_causal_ml_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use statistical_causal_ml_contract_support::{AnalysisContractError,AnalysisContractRequest as WorldgenStatisticalCausalMlContractRequest,AnalysisContractReceipt as WorldgenStatisticalCausalMlContractReceipt};

