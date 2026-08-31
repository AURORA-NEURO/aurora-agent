//! Prospective high-throughput context contract model (`AFA-worldgen-P03-F07`).
use super::context_contract_support::{self,ContextContractRequest,ContextContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F07";pub const CONTRACT_VERSION:&str="worldgen-throughput-context-contract/1.0";pub const INPUT_SCHEMA:&str="ContextContractRequest3@1";
pub fn worldgen_throughput_context_contract_manifest()->serde_json::Value{context_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"prospective high-throughput","A2")}
pub fn compile_worldgen_throughput_context_contract(r:&ContextContractRequest)->Result<ContextContractReceipt,context_contract_support::ContextContractError>{context_contract_support::compile(r,FEATURE_ID,CONTRACT_VERSION,true)}
pub use context_contract_support::{ContextContractReceipt as WorldgenThroughputContextContractReceipt,ContextContractRequest as WorldgenThroughputContextContractRequest};
