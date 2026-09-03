//! Federated continual context contract model (`AFA-worldgen-P03-F08`).
use super::context_contract_support::{self,ContextContractRequest,ContextContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F08";pub const CONTRACT_VERSION:&str="worldgen-federated-continual-context-contract/1.0";pub const INPUT_SCHEMA:&str="ContextContractRequest4@1";
pub fn worldgen_federated_continual_context_contract_manifest()->serde_json::Value{context_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual/autonomous","A2")}
pub fn compile_worldgen_federated_continual_context_contract(r:&ContextContractRequest)->Result<ContextContractReceipt,context_contract_support::ContextContractError>{context_contract_support::compile(r,FEATURE_ID,CONTRACT_VERSION,true)}
pub use context_contract_support::{ContextContractReceipt as WorldgenFederatedContinualContextContractReceipt,ContextContractRequest as WorldgenFederatedContinualContextContractRequest};
