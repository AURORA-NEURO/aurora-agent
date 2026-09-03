//! Adapter P32 federated continual contract-model gateway-integrity feature.
use super::gateway_integrity_support::{manifest,qualify,GatewayIntegrityCard7,GatewayIntegrityError,GatewayIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-adapter-P32-F14";pub const CONTRACT_VERSION:&str="adapter-federated_continual_gateway_integrity_contract_model/1.0";
pub fn federated_continual_gateway_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual","contract-model")}
pub fn qualify_federated_continual_gateway_integrity_contract_model(request:&GatewayIntegrityRequest4)->Result<GatewayIntegrityCard7,GatewayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"federated continual","contract-model")}
