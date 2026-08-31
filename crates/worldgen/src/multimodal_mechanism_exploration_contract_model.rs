//! Worldgen P08 AFA-worldgen-P08-F06 mechanism contract model.
use super::mechanism_contract_support::{self,MechanismContractRequest,MechanismContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-mechanism-contract/1.0";
pub fn worldgen_multimodal_mechanism_exploration_contract_model_manifest()->serde_json::Value{mechanism_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismContractRequest1@1","multimodal multi-study","A1")}
pub fn negotiate_worldgen_multimodal_mechanism_contract(request:&MechanismContractRequest)->Result<MechanismContractReceipt,mechanism_contract_support::MechanismContractError>{mechanism_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use mechanism_contract_support::{MechanismContractError,MechanismContractReceipt as WorldgenMultimodalMechanismcontractmodelReceipt,MechanismContractRequest as WorldgenMultimodalMechanismcontractmodelRequest};

