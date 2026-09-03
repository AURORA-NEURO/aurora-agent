//! Worldgen P07 AFA-worldgen-P07-F01 quality control inference.
use super::quality_control_support::{self,QualityControlRequest,QualityControlReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-quality-control/1.0";
pub fn worldgen_local_quality_control_inference_manifest()->serde_json::Value{quality_control_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityControlRequest1@1","local single-study","A0")}
pub fn assess_worldgen_local_quality_control(request:&QualityControlRequest)->Result<QualityControlReceipt,quality_control_support::QualityControlError>{quality_control_support::assess(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use quality_control_support::{QualityControlError,QualityControlReceipt as WorldgenLocalQualitycontrolinferenceReceipt,QualityControlRequest as WorldgenLocalQualitycontrolinferenceRequest};

