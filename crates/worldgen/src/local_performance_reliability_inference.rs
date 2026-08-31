//! Worldgen P21 AFA-worldgen-P21-F01 performance/reliability inference.
use super::performance_reliability_support::{self,CapabilityWorkloadRequest4,ReliableCapabilityResult6};
pub const FEATURE_ID:&str="AFA-worldgen-P21-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-performance-reliability/1.0";
pub fn worldgen_local_performance_reliability_inference_manifest()->serde_json::Value{let mut v=performance_reliability_support::performance_reliability_gateway_manifest();v["capability_id"]=serde_json::json!(FEATURE_ID);v["version"]=serde_json::json!(CONTRACT_VERSION);v}
pub fn assess_worldgen_local_performance_reliability(request:&CapabilityWorkloadRequest4)->Result<ReliableCapabilityResult6,performance_reliability_support::PerformanceReliabilityError>{let mut out=performance_reliability_support::assess_performance_reliability(request)?;out.feature_id=FEATURE_ID.into();out.contract_version=CONTRACT_VERSION.into();Ok(out)}



