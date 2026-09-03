//! Worldgen P22 local interoperability/extensibility inference.
use super::interoperability_extensibility_support::{self,ExtensibilityRequest4,ExtensibilityReceipt7};
pub const FEATURE_ID:&str="AFA-worldgen-P22-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-interoperability-extensibility/1.0";
pub fn worldgen_local_interoperability_extensibility_inference_manifest()->serde_json::Value{interoperability_extensibility_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn negotiate_worldgen_local_interoperability_extensibility(request:&ExtensibilityRequest4)->Result<ExtensibilityReceipt7,interoperability_extensibility_support::InteroperabilityExtensibilityError>{interoperability_extensibility_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
