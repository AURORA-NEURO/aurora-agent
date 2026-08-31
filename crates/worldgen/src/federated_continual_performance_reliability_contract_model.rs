//! Worldgen P21 AFA-worldgen-P21-F08 performance/reliability contract.
use super::performance_reliability_contract_support::{self,PerformanceReliabilityContractRequest,PerformanceReliabilityContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P21-F08"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-performance-reliability-contract/1.0";
pub fn worldgen_federated_continual_performance_reliability_contract_model_manifest()->serde_json::Value{performance_reliability_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn negotiate_worldgen_federated_continual_performance_reliability_contract(request:&PerformanceReliabilityContractRequest)->Result<PerformanceReliabilityContractReceipt,performance_reliability_contract_support::PerformanceReliabilityContractError>{performance_reliability_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}



