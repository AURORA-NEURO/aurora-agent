//! Bundle P32 local contract_model signed research-object integrity feature.
use super::research_bundle_integrity_support::{manifest,release,BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError};
pub const FEATURE_ID:&str="AFA-bundle-P32-F05";pub const CONTRACT_VERSION:&str="bundle-local_research_bundle_integrity_contract_model/1.0";
pub fn local_research_bundle_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
pub fn release_local_research_bundle_integrity_contract_model(request:&BundleReleaseRequest4)->Result<BundleCard7,ResearchBundleIntegrityError>{release(request,FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
