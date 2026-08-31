//! Worldgen P08 AFA-worldgen-P08-F08 mechanism contract model.
use super::mechanism_contract_support::{self,MechanismContractRequest,MechanismContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F08"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-mechanism-contract/1.0";
pub fn worldgen_federated_continual_mechanism_exploration_contract_model_manifest()->serde_json::Value{mechanism_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismContractRequest1@1","federated continual autonomous","A1")}
pub fn negotiate_worldgen_federated_continual_mechanism_contract(request:&MechanismContractRequest)->Result<MechanismContractReceipt,mechanism_contract_support::MechanismContractError>{mechanism_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use mechanism_contract_support::{MechanismContractError,MechanismContractReceipt as WorldgenFederatedContinualMechanismcontractmodelReceipt,MechanismContractRequest as WorldgenFederatedContinualMechanismcontractmodelRequest};

