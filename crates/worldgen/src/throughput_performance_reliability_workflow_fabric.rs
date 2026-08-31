//! Worldgen P21 AFA-worldgen-P21-F15 performance/reliability workflow.
use super::performance_reliability_workflow_support::{self,PerformanceReliabilityWorkflowRequest,PerformanceReliabilityWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P21-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-performance-reliability-workflow/1.0";
pub fn worldgen_throughput_performance_reliability_workflow_fabric_manifest()->serde_json::Value{performance_reliability_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn schedule_worldgen_throughput_performance_reliability_workflow(request:&PerformanceReliabilityWorkflowRequest)->Result<PerformanceReliabilityWorkflowReceipt,performance_reliability_workflow_support::PerformanceReliabilityWorkflowError>{performance_reliability_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true)}



