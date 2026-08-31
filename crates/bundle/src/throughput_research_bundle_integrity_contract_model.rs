//! Bundle P32 throughput contract_model signed research-object integrity feature.
use super::research_bundle_integrity_support::{manifest,release,BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError};
pub const FEATURE_ID:&str="AFA-bundle-P32-F07";pub const CONTRACT_VERSION:&str="bundle-throughput_research_bundle_integrity_contract_model/1.0";
pub fn throughput_research_bundle_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","contract_model")}
pub fn release_throughput_research_bundle_integrity_contract_model(request:&BundleReleaseRequest4)->Result<BundleCard7,ResearchBundleIntegrityError>{release(request,FEATURE_ID,CONTRACT_VERSION,"throughput","contract_model")}
