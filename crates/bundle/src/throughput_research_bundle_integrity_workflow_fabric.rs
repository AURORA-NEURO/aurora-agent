//! Bundle P32 throughput workflow_fabric signed research-object integrity feature.
use super::research_bundle_integrity_support::{manifest,release,BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError};
pub const FEATURE_ID:&str="AFA-bundle-P32-F15";pub const CONTRACT_VERSION:&str="bundle-throughput_research_bundle_integrity_workflow_fabric/1.0";
pub fn throughput_research_bundle_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","workflow_fabric")}
pub fn release_throughput_research_bundle_integrity_workflow_fabric(request:&BundleReleaseRequest4)->Result<BundleCard7,ResearchBundleIntegrityError>{release(request,FEATURE_ID,CONTRACT_VERSION,"throughput","workflow_fabric")}
