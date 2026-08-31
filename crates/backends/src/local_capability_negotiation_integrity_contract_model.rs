//! Backends P32 local contract_model capability-negotiation integrity feature.
use super::capability_negotiation_integrity_support::{manifest,negotiate,BackendCard7,BackendRequest4,CapabilityNegotiationIntegrityError};
pub const FEATURE_ID:&str="AFA-backends-P32-F05";pub const CONTRACT_VERSION:&str="backends-local_capability_negotiation_integrity_contract_model/1.0";
pub fn local_capability_negotiation_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
pub fn negotiate_local_capability_negotiation_integrity_contract_model(request:&BackendRequest4)->Result<BackendCard7,CapabilityNegotiationIntegrityError>{negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
