//! Worldgen P07 AFA-worldgen-P07-F03 quality control inference.
use super::quality_control_support::{self,QualityControlRequest,QualityControlReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F03"; pub const CONTRACT_VERSION:&str="worldgen-throughput-quality-control/1.0";
pub fn worldgen_throughput_quality_control_inference_manifest()->serde_json::Value{quality_control_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityControlRequest1@1","prospective high-throughput","A1")}
pub fn assess_worldgen_throughput_quality_control(request:&QualityControlRequest)->Result<QualityControlReceipt,quality_control_support::QualityControlError>{quality_control_support::assess(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use quality_control_support::{QualityControlError,QualityControlReceipt as WorldgenThroughputQualitycontrolinferenceReceipt,QualityControlRequest as WorldgenThroughputQualitycontrolinferenceRequest};

