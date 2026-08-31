//! Worldgen P08 AFA-worldgen-P08-F05 mechanism contract model.
use super::mechanism_contract_support::{self,MechanismContractRequest,MechanismContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-mechanism-contract/1.0";
pub fn worldgen_local_mechanism_exploration_contract_model_manifest()->serde_json::Value{mechanism_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismContractRequest1@1","local single-study","A0")}
pub fn negotiate_worldgen_local_mechanism_contract(request:&MechanismContractRequest)->Result<MechanismContractReceipt,mechanism_contract_support::MechanismContractError>{mechanism_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use mechanism_contract_support::{MechanismContractError,MechanismContractReceipt as WorldgenLocalMechanismcontractmodelReceipt,MechanismContractRequest as WorldgenLocalMechanismcontractmodelRequest};

