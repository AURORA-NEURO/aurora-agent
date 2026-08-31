//! Worldgen P21 AFA-worldgen-P21-F09 performance/reliability copilot.
use super::performance_reliability_copilot_support::{self,PerformanceReliabilityCopilotRequest,PerformanceReliabilityCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P21-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-performance-reliability-copilot/1.0";
pub fn worldgen_local_performance_reliability_research_copilot_manifest()->serde_json::Value{performance_reliability_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn run_worldgen_local_performance_reliability_research_copilot(request:&PerformanceReliabilityCopilotRequest)->Result<PerformanceReliabilityCopilotReceipt,performance_reliability_copilot_support::PerformanceReliabilityCopilotError>{performance_reliability_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true)}



