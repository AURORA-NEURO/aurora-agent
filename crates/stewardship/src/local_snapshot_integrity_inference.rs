//! Stewardship P32 local single-study inference snapshot-integrity feature F01.
use super::snapshot_integrity_support::{qualify,manifest,SnapshotIntegrityCard7,SnapshotIntegrityRequest4,SnapshotIntegrityError};
const FEATURE_ID:&str="AFA-stewardship-P32-F01";const CONTRACT_VERSION:&str="stewardship-local-snapshot-integrity-inference/1.0";
pub fn stewardship_local_snapshot_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_stewardship_local_snapshot_integrity_inference(request:&SnapshotIntegrityRequest4)->Result<SnapshotIntegrityCard7,SnapshotIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
