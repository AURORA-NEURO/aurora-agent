//! Worldgen P21 AFA-worldgen-P21-F10 performance/reliability copilot.
use super::performance_reliability_copilot_support::{self,PerformanceReliabilityCopilotRequest,PerformanceReliabilityCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P21-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-performance-reliability-copilot/1.0";
pub fn worldgen_multimodal_performance_reliability_research_copilot_manifest()->serde_json::Value{performance_reliability_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn run_worldgen_multimodal_performance_reliability_research_copilot(request:&PerformanceReliabilityCopilotRequest)->Result<PerformanceReliabilityCopilotReceipt,performance_reliability_copilot_support::PerformanceReliabilityCopilotError>{performance_reliability_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true)}



