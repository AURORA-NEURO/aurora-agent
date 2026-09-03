//! Adapter P32 multimodal workflow-fabric gateway-integrity feature.
use super::gateway_integrity_support::{manifest,qualify,GatewayIntegrityCard7,GatewayIntegrityError,GatewayIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-adapter-P32-F08";pub const CONTRACT_VERSION:&str="adapter-multimodal_gateway_integrity_workflow_fabric/1.0";
pub fn multimodal_gateway_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","workflow-fabric")}
pub fn qualify_multimodal_gateway_integrity_workflow_fabric(request:&GatewayIntegrityRequest4)->Result<GatewayIntegrityCard7,GatewayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","workflow-fabric")}
