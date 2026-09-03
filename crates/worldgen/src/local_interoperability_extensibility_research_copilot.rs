//! Worldgen P22 local interoperability/extensibility research copilot.
use super::interoperability_extensibility_support::{self,ExtensibilityRequest4,ExtensibilityReceipt7};
pub const FEATURE_ID:&str="AFA-worldgen-P22-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-interoperability-extensibility-copilot/1.0";
pub fn worldgen_local_interoperability_extensibility_research_copilot_manifest()->serde_json::Value{interoperability_extensibility_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","copilot")}
pub fn run_worldgen_local_interoperability_extensibility_research_copilot(request:&ExtensibilityRequest4)->Result<ExtensibilityReceipt7,interoperability_extensibility_support::InteroperabilityExtensibilityError>{interoperability_extensibility_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","copilot")}
