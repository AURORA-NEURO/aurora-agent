//! Worldgen P04-F06 multimodal knowledge-representation contract model.
use super::knowledge_contract_support::{self, KnowledgeContractRequest, KnowledgeContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-knowledge-contract/1.0";
pub fn worldgen_multimodal_knowledge_representation_contract_model_manifest()->serde_json::Value{knowledge_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeContractRequest1@1","multimodal multi-study","A1")}
pub fn negotiate_worldgen_multimodal_knowledge_contract(request:&KnowledgeContractRequest)->Result<KnowledgeContractReceipt,knowledge_contract_support::KnowledgeContractError>{knowledge_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use knowledge_contract_support::{KnowledgeContractError,KnowledgeContractReceipt as WorldgenMultimodalKnowledgeContractReceipt,KnowledgeContractRequest as WorldgenMultimodalKnowledgeContractRequest};
