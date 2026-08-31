//! Backends P32 throughput inference capability-negotiation integrity feature.
use super::capability_negotiation_integrity_support::{manifest,negotiate,BackendCard7,BackendRequest4,CapabilityNegotiationIntegrityError};
pub const FEATURE_ID:&str="AFA-backends-P32-F03";pub const CONTRACT_VERSION:&str="backends-throughput_capability_negotiation_integrity_inference/1.0";
pub fn throughput_capability_negotiation_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
pub fn negotiate_throughput_capability_negotiation_integrity_inference(request:&BackendRequest4)->Result<BackendCard7,CapabilityNegotiationIntegrityError>{negotiate(request,FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
