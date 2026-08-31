//! Worldgen P09 AFA-worldgen-P09-F08 experiment_design contract model.
use super::experiment_design_contract_support::{self,ExperimentDesignContractRequest,ExperimentDesignContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P09-F08"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-experiment_design-contract/1.0";
pub fn worldgen_federated_continual_experiment_design_contract_model_manifest()->serde_json::Value{experiment_design_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ExperimentDesignContractRequest1@1","federated continual autonomous","A1")}
pub fn negotiate_worldgen_federated_continual_experiment_design_contract(request:&ExperimentDesignContractRequest)->Result<ExperimentDesignContractReceipt,experiment_design_contract_support::ExperimentDesignContractError>{experiment_design_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use experiment_design_contract_support::{ExperimentDesignContractError,ExperimentDesignContractReceipt as WorldgenFederatedContinualExperimentDesigncontractmodelReceipt,ExperimentDesignContractRequest as WorldgenFederatedContinualExperimentDesigncontractmodelRequest};

