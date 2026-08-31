//! Worldgen P09 AFA-worldgen-P09-F11 experiment_design research copilot.
use super::experiment_design_copilot_support::{self,ExperimentDesignCopilotRequest,ExperimentDesignCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P09-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-experiment_design-copilot/1.0";
pub fn worldgen_throughput_experiment_design_research_copilot_manifest()->serde_json::Value{experiment_design_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ExperimentDesignCopilotRequest1@1","prospective high-throughput","A1")}
pub fn run_worldgen_throughput_experiment_design_research_copilot(request:&ExperimentDesignCopilotRequest)->Result<ExperimentDesignCopilotReceipt,experiment_design_copilot_support::ExperimentDesignCopilotError>{experiment_design_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false,false)}
pub use experiment_design_copilot_support::{ExperimentDesignCopilotError,ExperimentDesignCopilotReceipt as WorldgenThroughputExperimentDesignresearchcopilotReceipt,ExperimentDesignCopilotRequest as WorldgenThroughputExperimentDesignresearchcopilotRequest};

