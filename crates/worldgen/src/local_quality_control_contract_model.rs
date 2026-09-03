//! Worldgen P07 AFA-worldgen-P07-F05 quality contract model.
use super::quality_contract_support::{self,QualityContractRequest,QualityContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-quality-contract/1.0";
pub fn worldgen_local_quality_control_contract_model_manifest()->serde_json::Value{quality_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityContractRequest1@1","local single-study","A0")}
pub fn negotiate_worldgen_local_quality_contract(request:&QualityContractRequest)->Result<QualityContractReceipt,quality_contract_support::QualityContractError>{quality_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use quality_contract_support::{QualityContractError,QualityContractReceipt as WorldgenLocalQualitycontractmodelReceipt,QualityContractRequest as WorldgenLocalQualitycontractmodelRequest};

