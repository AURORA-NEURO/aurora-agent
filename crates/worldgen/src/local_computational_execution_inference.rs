//! Worldgen P12 AFA-worldgen-P12-F01 computational_execution exploration inference.
use super::computational_execution_support::{self,ResearchWorkflowSpec3,ExecutionRun7};
pub const FEATURE_ID:&str="AFA-worldgen-P12-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-computational_execution-exploration/1.0";
pub fn worldgen_local_computational_execution_inference_manifest()->serde_json::Value{computational_execution_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn assure_computational_execution_worldgen_local_computational_executions(request:&ResearchWorkflowSpec3)->Result<ExecutionRun7,computational_execution_support::ComputationalExecutionError>{computational_execution_support::assure_computational_execution(request,FEATURE_ID,CONTRACT_VERSION)}
pub use computational_execution_support::{ExecutionNode3,ComputationalExecutionError,ExecutionRun7 as WorldgenLocalComputationalExecutionportfolioInference,ResearchWorkflowSpec3 as WorldgenLocalComputationalExecutionquestionInference};

