//! Worldgen P07 AFA-worldgen-P07-F06 quality contract model.
use super::quality_contract_support::{self,QualityContractRequest,QualityContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-quality-contract/1.0";
pub fn worldgen_multimodal_quality_control_contract_model_manifest()->serde_json::Value{quality_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityContractRequest1@1","multimodal multi-study","A1")}
pub fn negotiate_worldgen_multimodal_quality_contract(request:&QualityContractRequest)->Result<QualityContractReceipt,quality_contract_support::QualityContractError>{quality_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use quality_contract_support::{QualityContractError,QualityContractReceipt as WorldgenMultimodalQualitycontractmodelReceipt,QualityContractRequest as WorldgenMultimodalQualitycontractmodelRequest};

