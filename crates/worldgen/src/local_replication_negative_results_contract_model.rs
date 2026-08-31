//! Worldgen P15 F05 statistical, causal, and ML contract model.
use super::replication_negative_results_contract_support::{self,ReplicationContractRequest,ReplicationContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P15-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-replication-negative-results-contract/1.0";
pub fn worldgen_local_replication_negative_results_contract_model_manifest()->serde_json::Value{replication_negative_results_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn negotiate_worldgen_local_replication_negative_results_contract(request:&ReplicationContractRequest)->Result<ReplicationContractReceipt,replication_negative_results_contract_support::ReplicationContractError>{replication_negative_results_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use replication_negative_results_contract_support::{ReplicationContractError,ReplicationContractRequest as WorldgenReplicationNegativeResultsContractRequest,ReplicationContractReceipt as WorldgenReplicationNegativeResultsContractReceipt};

