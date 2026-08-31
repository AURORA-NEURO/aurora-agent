//! Adapter P32 local research-copilot gateway-integrity feature.
use super::gateway_integrity_support::{manifest,qualify,GatewayIntegrityCard7,GatewayIntegrityError,GatewayIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-adapter-P32-F03";pub const CONTRACT_VERSION:&str="adapter-local_gateway_integrity_research_copilot/1.0";
pub fn local_gateway_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","research-copilot")}
pub fn qualify_local_gateway_integrity_research_copilot(request:&GatewayIntegrityRequest4)->Result<GatewayIntegrityCard7,GatewayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","research-copilot")}
