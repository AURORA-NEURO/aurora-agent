//! Worldgen P21 AFA-worldgen-P21-F12 performance/reliability copilot.
use super::performance_reliability_copilot_support::{self,PerformanceReliabilityCopilotRequest,PerformanceReliabilityCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P21-F12"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-performance-reliability-copilot/1.0";
pub fn worldgen_federated_continual_performance_reliability_research_copilot_manifest()->serde_json::Value{performance_reliability_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn run_worldgen_federated_continual_performance_reliability_research_copilot(request:&PerformanceReliabilityCopilotRequest)->Result<PerformanceReliabilityCopilotReceipt,performance_reliability_copilot_support::PerformanceReliabilityCopilotError>{performance_reliability_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}



