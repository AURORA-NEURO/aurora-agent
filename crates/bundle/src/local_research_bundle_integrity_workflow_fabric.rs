//! Bundle P32 local workflow_fabric signed research-object integrity feature.
use super::research_bundle_integrity_support::{manifest,release,BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError};
pub const FEATURE_ID:&str="AFA-bundle-P32-F13";pub const CONTRACT_VERSION:&str="bundle-local_research_bundle_integrity_workflow_fabric/1.0";
pub fn local_research_bundle_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","workflow_fabric")}
pub fn release_local_research_bundle_integrity_workflow_fabric(request:&BundleReleaseRequest4)->Result<BundleCard7,ResearchBundleIntegrityError>{release(request,FEATURE_ID,CONTRACT_VERSION,"local","workflow_fabric")}
