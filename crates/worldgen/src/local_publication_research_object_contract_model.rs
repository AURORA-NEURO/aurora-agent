//! Worldgen P16 F05 statistical, causal, and ML contract model.
use super::publication_research_object_contract_support::{self,ReleaseContractRequest,ReleaseContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P16-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-publication-research-object-contract/1.0";
pub fn worldgen_local_publication_research_object_contract_model_manifest()->serde_json::Value{publication_research_object_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn negotiate_worldgen_local_publication_research_object_contract(request:&ReleaseContractRequest)->Result<ReleaseContractReceipt,publication_research_object_contract_support::ReleaseContractError>{publication_research_object_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use publication_research_object_contract_support::{ReleaseContractError,ReleaseContractRequest as WorldgenPublicationResearchObjectContractRequest,ReleaseContractReceipt as WorldgenPublicationResearchObjectContractReceipt};

