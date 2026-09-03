//! Worldgen P15 F07 statistical, causal, and ML contract model.
use super::replication_negative_results_contract_support::{self,ReplicationContractRequest,ReplicationContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P15-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-replication-negative-results-contract/1.0";
pub fn worldgen_throughput_replication_negative_results_contract_model_manifest()->serde_json::Value{replication_negative_results_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn negotiate_worldgen_throughput_replication_negative_results_contract(request:&ReplicationContractRequest)->Result<ReplicationContractReceipt,replication_negative_results_contract_support::ReplicationContractError>{replication_negative_results_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use replication_negative_results_contract_support::{ReplicationContractError,ReplicationContractRequest as WorldgenReplicationNegativeResultsContractRequest,ReplicationContractReceipt as WorldgenReplicationNegativeResultsContractReceipt};

