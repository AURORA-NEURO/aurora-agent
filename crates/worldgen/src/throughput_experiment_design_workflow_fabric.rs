//! Worldgen P09 AFA-worldgen-P09-F15 experiment_design workflow fabric.
use super::experiment_design_workflow_support::{self,ExperimentDesignWorkflowRequest,ExperimentDesignWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P09-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-experiment_design-workflow/1.0";
pub fn worldgen_throughput_experiment_design_workflow_fabric_manifest()->serde_json::Value{experiment_design_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ExperimentDesignWorkflowRequest1@1","prospective high-throughput","A1")}
pub fn schedule_worldgen_throughput_experiment_design_workflow(request:&ExperimentDesignWorkflowRequest)->Result<ExperimentDesignWorkflowReceipt,experiment_design_workflow_support::ExperimentDesignWorkflowError>{experiment_design_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false,false)}
pub use experiment_design_workflow_support::{ExperimentDesignWorkflowError,ExperimentDesignWorkflowReceipt as WorldgenThroughputExperimentDesignworkflowfabricReceipt,ExperimentDesignWorkflowRequest as WorldgenThroughputExperimentDesignworkflowfabricRequest};

