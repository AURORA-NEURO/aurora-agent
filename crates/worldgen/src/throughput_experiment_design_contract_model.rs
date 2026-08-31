//! Worldgen P09 AFA-worldgen-P09-F07 experiment_design contract model.
use super::experiment_design_contract_support::{self,ExperimentDesignContractRequest,ExperimentDesignContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P09-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-experiment_design-contract/1.0";
pub fn worldgen_throughput_experiment_design_contract_model_manifest()->serde_json::Value{experiment_design_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ExperimentDesignContractRequest1@1","prospective high-throughput","A1")}
pub fn negotiate_worldgen_throughput_experiment_design_contract(request:&ExperimentDesignContractRequest)->Result<ExperimentDesignContractReceipt,experiment_design_contract_support::ExperimentDesignContractError>{experiment_design_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use experiment_design_contract_support::{ExperimentDesignContractError,ExperimentDesignContractReceipt as WorldgenThroughputExperimentDesigncontractmodelReceipt,ExperimentDesignContractRequest as WorldgenThroughputExperimentDesigncontractmodelRequest};

