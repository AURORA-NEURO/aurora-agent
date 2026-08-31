//! Worldgen P22 multimodal interoperability/extensibility workflow fabric.
use super::interoperability_extensibility_support::{self,ExtensibilityRequest4,ExtensibilityReceipt7};
pub const FEATURE_ID:&str="AFA-worldgen-P22-F14"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-interoperability-extensibility-workflow/1.0";
pub fn worldgen_multimodal_interoperability_extensibility_workflow_fabric_manifest()->serde_json::Value{interoperability_extensibility_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow")}
pub fn schedule_worldgen_multimodal_interoperability_extensibility_workflow(request:&ExtensibilityRequest4)->Result<ExtensibilityReceipt7,interoperability_extensibility_support::InteroperabilityExtensibilityError>{interoperability_extensibility_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow")}
