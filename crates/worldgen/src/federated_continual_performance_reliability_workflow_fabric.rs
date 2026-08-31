//! Worldgen P21 AFA-worldgen-P21-F16 performance/reliability workflow.
use super::performance_reliability_workflow_support::{self,PerformanceReliabilityWorkflowRequest,PerformanceReliabilityWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P21-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-performance-reliability-workflow/1.0";
pub fn worldgen_federated_continual_performance_reliability_workflow_fabric_manifest()->serde_json::Value{performance_reliability_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn schedule_worldgen_federated_continual_performance_reliability_workflow(request:&PerformanceReliabilityWorkflowRequest)->Result<PerformanceReliabilityWorkflowReceipt,performance_reliability_workflow_support::PerformanceReliabilityWorkflowError>{performance_reliability_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}



