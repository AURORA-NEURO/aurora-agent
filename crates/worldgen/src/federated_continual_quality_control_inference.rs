//! Worldgen P07 AFA-worldgen-P07-F04 quality control inference.
use super::quality_control_support::{self,QualityControlRequest,QualityControlReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-quality-control/1.0";
pub fn worldgen_federated_continual_quality_control_inference_manifest()->serde_json::Value{quality_control_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityControlRequest1@1","federated continual autonomous","A1")}
pub fn assess_worldgen_federated_continual_quality_control(request:&QualityControlRequest)->Result<QualityControlReceipt,quality_control_support::QualityControlError>{quality_control_support::assess(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use quality_control_support::{QualityControlError,QualityControlReceipt as WorldgenFederatedContinualQualitycontrolinferenceReceipt,QualityControlRequest as WorldgenFederatedContinualQualitycontrolinferenceRequest};

