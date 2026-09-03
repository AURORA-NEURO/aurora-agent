//! Worldgen P19 F07 statistical, causal, and ML contract model.
use super::policy_autonomy_contract_support::{self,PolicyAutonomyContractRequest,PolicyAutonomyContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P19-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-policy_autonomy-signing-contract/1.0";
pub fn worldgen_throughput_policy_autonomy_contract_model_manifest()->serde_json::Value{policy_autonomy_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn negotiate_worldgen_throughput_policy_autonomy_contract(request:&PolicyAutonomyContractRequest)->Result<PolicyAutonomyContractReceipt,policy_autonomy_contract_support::PolicyAutonomyContractError>{policy_autonomy_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use policy_autonomy_contract_support::{PolicyAutonomyContractError,PolicyAutonomyContractRequest as WorldgenTypedPolicyAutonomyContractRequest,PolicyAutonomyContractReceipt as WorldgenTypedPolicyAutonomyContractReceipt};

