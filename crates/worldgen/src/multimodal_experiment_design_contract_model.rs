//! Worldgen P09 AFA-worldgen-P09-F06 experiment_design contract model.
use super::experiment_design_contract_support::{self,ExperimentDesignContractRequest,ExperimentDesignContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P09-F06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-experiment_design-contract/1.0";
pub fn worldgen_multimodal_experiment_design_contract_model_manifest()->serde_json::Value{experiment_design_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ExperimentDesignContractRequest1@1","multimodal multi-study","A1")}
pub fn negotiate_worldgen_multimodal_experiment_design_contract(request:&ExperimentDesignContractRequest)->Result<ExperimentDesignContractReceipt,experiment_design_contract_support::ExperimentDesignContractError>{experiment_design_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use experiment_design_contract_support::{ExperimentDesignContractError,ExperimentDesignContractReceipt as WorldgenMultimodalExperimentDesigncontractmodelReceipt,ExperimentDesignContractRequest as WorldgenMultimodalExperimentDesigncontractmodelRequest};

