//! Worldgen P14 F07 statistical, causal, and ML contract model.
use super::interpretation_visualization_contract_support::{self,InterpretationContractRequest,InterpretationContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P14-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-interpretation-visualization-contract/1.0";
pub fn worldgen_throughput_interpretation_visualization_contract_model_manifest()->serde_json::Value{interpretation_visualization_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn negotiate_worldgen_throughput_interpretation_visualization_contract(request:&InterpretationContractRequest)->Result<InterpretationContractReceipt,interpretation_visualization_contract_support::InterpretationContractError>{interpretation_visualization_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use interpretation_visualization_contract_support::{InterpretationContractError,InterpretationContractRequest as WorldgenInterpretationVisualizationContractRequest,InterpretationContractReceipt as WorldgenInterpretationVisualizationContractReceipt};

