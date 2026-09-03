//! Local context contract model (`AFA-worldgen-P03-F05`).
use super::context_contract_support::{self,ContextContractRequest,ContextContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F05";pub const CONTRACT_VERSION:&str="worldgen-local-context-contract/1.0";pub const INPUT_SCHEMA:&str="ContextContractRequest1@1";
pub fn worldgen_local_context_contract_manifest()->serde_json::Value{context_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"local single-study","A0")}
pub fn compile_worldgen_local_context_contract(r:&ContextContractRequest)->Result<ContextContractReceipt,context_contract_support::ContextContractError>{context_contract_support::compile(r,FEATURE_ID,CONTRACT_VERSION,false)}
pub use context_contract_support::{ContextContractError,ContextContractReceipt as WorldgenLocalContextContractReceipt,ContextContractRequest as WorldgenLocalContextContractRequest};
