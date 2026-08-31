//! Bundle P32 federated_continual research_copilot signed research-object integrity feature.
use super::research_bundle_integrity_support::{manifest,release,BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError};
pub const FEATURE_ID:&str="AFA-bundle-P32-F12";pub const CONTRACT_VERSION:&str="bundle-federated_continual_research_bundle_integrity_research_copilot/1.0";
pub fn federated_continual_research_bundle_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated_continual","research_copilot")}
pub fn release_federated_continual_research_bundle_integrity_research_copilot(request:&BundleReleaseRequest4)->Result<BundleCard7,ResearchBundleIntegrityError>{release(request,FEATURE_ID,CONTRACT_VERSION,"federated_continual","research_copilot")}
