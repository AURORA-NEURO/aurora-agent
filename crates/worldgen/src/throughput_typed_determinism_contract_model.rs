//! Worldgen P17 F07 statistical, causal, and ML contract model.
use super::typed_determinism_contract_support::{self,DeterminismContractRequest,DeterminismContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P17-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-typed-determinism-contract/1.0";
pub fn worldgen_throughput_typed_determinism_contract_model_manifest()->serde_json::Value{typed_determinism_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn negotiate_worldgen_throughput_typed_determinism_contract(request:&DeterminismContractRequest)->Result<DeterminismContractReceipt,typed_determinism_contract_support::DeterminismContractError>{typed_determinism_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use typed_determinism_contract_support::{DeterminismContractError,DeterminismContractRequest as WorldgenTypedDeterminismContractRequest,DeterminismContractReceipt as WorldgenTypedDeterminismContractReceipt};

