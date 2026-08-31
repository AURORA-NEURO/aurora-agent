//! Backends P32 multimodal workflow_fabric capability-negotiation integrity feature.
use super::capability_negotiation_integrity_support::{manifest,negotiate,BackendCard7,BackendRequest4,CapabilityNegotiationIntegrityError};
pub const FEATURE_ID:&str="AFA-backends-P32-F14";pub const CONTRACT_VERSION:&str="backends-multimodal_capability_negotiation_integrity_workflow_fabric/1.0";
pub fn multimodal_capability_negotiation_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","workflow_fabric")}
pub fn negotiate_multimodal_capability_negotiation_integrity_workflow_fabric(request:&BackendRequest4)->Result<BackendCard7,CapabilityNegotiationIntegrityError>{negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","workflow_fabric")}
