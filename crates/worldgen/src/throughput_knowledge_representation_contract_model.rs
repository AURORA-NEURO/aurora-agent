//! Worldgen P04-F07 prospective high-throughput knowledge-representation contract model.
use super::knowledge_contract_support::{self, KnowledgeContractRequest, KnowledgeContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-knowledge-contract/1.0";
pub fn worldgen_throughput_knowledge_representation_contract_model_manifest()->serde_json::Value{knowledge_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeContractRequest1@1","prospective high-throughput","A1")}
pub fn negotiate_worldgen_throughput_knowledge_contract(request:&KnowledgeContractRequest)->Result<KnowledgeContractReceipt,knowledge_contract_support::KnowledgeContractError>{knowledge_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true)}
pub use knowledge_contract_support::{KnowledgeContractError,KnowledgeContractReceipt as WorldgenThroughputKnowledgeContractReceipt,KnowledgeContractRequest as WorldgenThroughputKnowledgeContractRequest};
