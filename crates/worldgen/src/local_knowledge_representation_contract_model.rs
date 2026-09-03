//! Worldgen P04-F05 local knowledge-representation contract model.
use super::knowledge_contract_support::{self, KnowledgeContractRequest, KnowledgeContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-knowledge-contract/1.0";
pub fn worldgen_local_knowledge_representation_contract_model_manifest()->serde_json::Value{knowledge_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeContractRequest1@1","local single-study","A0")}
pub fn negotiate_worldgen_local_knowledge_contract(request:&KnowledgeContractRequest)->Result<KnowledgeContractReceipt,knowledge_contract_support::KnowledgeContractError>{knowledge_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use knowledge_contract_support::{KnowledgeContractError,KnowledgeContractReceipt as WorldgenLocalKnowledgeContractReceipt,KnowledgeContractRequest as WorldgenLocalKnowledgeContractRequest};
