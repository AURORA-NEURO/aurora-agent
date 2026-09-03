//! Worldgen P15 F06 statistical, causal, and ML contract model.
use super::replication_negative_results_contract_support::{self,ReplicationContractRequest,ReplicationContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P15-F06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-replication-negative-results-contract/1.0";
pub fn worldgen_multimodal_replication_negative_results_contract_model_manifest()->serde_json::Value{replication_negative_results_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn negotiate_worldgen_multimodal_replication_negative_results_contract(request:&ReplicationContractRequest)->Result<ReplicationContractReceipt,replication_negative_results_contract_support::ReplicationContractError>{replication_negative_results_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use replication_negative_results_contract_support::{ReplicationContractError,ReplicationContractRequest as WorldgenReplicationNegativeResultsContractRequest,ReplicationContractReceipt as WorldgenReplicationNegativeResultsContractReceipt};

