//! Multimodal context contract model (`AFA-worldgen-P03-F06`).
use super::context_contract_support::{self,ContextContractRequest,ContextContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F06";pub const CONTRACT_VERSION:&str="worldgen-multimodal-context-contract/1.0";pub const INPUT_SCHEMA:&str="ContextContractRequest2@1";
pub fn worldgen_multimodal_context_contract_manifest()->serde_json::Value{context_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"multimodal multi-study","A1")}
pub fn compile_worldgen_multimodal_context_contract(r:&ContextContractRequest)->Result<ContextContractReceipt,context_contract_support::ContextContractError>{context_contract_support::compile(r,FEATURE_ID,CONTRACT_VERSION,false)}
pub use context_contract_support::{ContextContractReceipt as WorldgenMultimodalContextContractReceipt,ContextContractRequest as WorldgenMultimodalContextContractRequest};
