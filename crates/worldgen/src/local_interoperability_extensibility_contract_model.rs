//! Worldgen P22 local interoperability/extensibility contract model.
use super::interoperability_extensibility_support::{self,ExtensibilityRequest4,ExtensibilityReceipt7};
pub const FEATURE_ID:&str="AFA-worldgen-P22-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-interoperability-extensibility-contract/1.0";
pub fn worldgen_local_interoperability_extensibility_contract_model_manifest()->serde_json::Value{interoperability_extensibility_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract")}
pub fn negotiate_worldgen_local_interoperability_extensibility_contract(request:&ExtensibilityRequest4)->Result<ExtensibilityReceipt7,interoperability_extensibility_support::InteroperabilityExtensibilityError>{interoperability_extensibility_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract")}
