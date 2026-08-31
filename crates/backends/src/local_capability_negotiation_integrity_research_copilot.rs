//! Backends P32 local research_copilot capability-negotiation integrity feature.
use super::capability_negotiation_integrity_support::{manifest,negotiate,BackendCard7,BackendRequest4,CapabilityNegotiationIntegrityError};
pub const FEATURE_ID:&str="AFA-backends-P32-F09";pub const CONTRACT_VERSION:&str="backends-local_capability_negotiation_integrity_research_copilot/1.0";
pub fn local_capability_negotiation_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
pub fn negotiate_local_capability_negotiation_integrity_research_copilot(request:&BackendRequest4)->Result<BackendCard7,CapabilityNegotiationIntegrityError>{negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
