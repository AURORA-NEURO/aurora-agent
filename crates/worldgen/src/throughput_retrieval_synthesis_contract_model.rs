//! Prospective throughput typed retrieval-synthesis contract model (`AFA-worldgen-P02-F07`).
use super::retrieval_contract_support::{self, RetrievalContractReceipt, RetrievalContractRequest};
pub const FEATURE_ID: &str = "AFA-worldgen-P02-F07";
pub const CONTRACT_VERSION: &str = "worldgen-throughput-retrieval-synthesis-contract/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub fn worldgen_throughput_retrieval_synthesis_contract_model_manifest() -> serde_json::Value { retrieval_contract_support::manifest(FEATURE_ID, CONTRACT_VERSION, INPUT_SCHEMA, "prospective high-throughput", "A1") }
pub fn compile_worldgen_throughput_retrieval_synthesis_contract(request: &RetrievalContractRequest) -> Result<RetrievalContractReceipt, retrieval_contract_support::RetrievalContractError> { retrieval_contract_support::compile(request, FEATURE_ID, CONTRACT_VERSION, INPUT_SCHEMA) }
pub use retrieval_contract_support::{RetrievalContractError, RetrievalContractReceipt as WorldgenThroughputRetrievalContractReceipt, RetrievalContractRequest as WorldgenThroughputRetrievalContractRequest};
