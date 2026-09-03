//! Adapter P32 throughput inference gateway-integrity feature.
use super::gateway_integrity_support::{manifest,qualify,GatewayIntegrityCard7,GatewayIntegrityError,GatewayIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-adapter-P32-F09";pub const CONTRACT_VERSION:&str="adapter-throughput_gateway_integrity_inference/1.0";
pub fn throughput_gateway_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
pub fn qualify_throughput_gateway_integrity_inference(request:&GatewayIntegrityRequest4)->Result<GatewayIntegrityCard7,GatewayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
