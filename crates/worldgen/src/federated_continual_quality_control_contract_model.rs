//! Worldgen P07 AFA-worldgen-P07-F08 quality contract model.
use super::quality_contract_support::{self,QualityContractRequest,QualityContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F08"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-quality-contract/1.0";
pub fn worldgen_federated_continual_quality_control_contract_model_manifest()->serde_json::Value{quality_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityContractRequest1@1","federated continual autonomous","A1")}
pub fn negotiate_worldgen_federated_continual_quality_contract(request:&QualityContractRequest)->Result<QualityContractReceipt,quality_contract_support::QualityContractError>{quality_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use quality_contract_support::{QualityContractError,QualityContractReceipt as WorldgenFederatedContinualQualitycontractmodelReceipt,QualityContractRequest as WorldgenFederatedContinualQualitycontractmodelRequest};

