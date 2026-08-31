//! Stewardship P32 multimodal multi-study contract_model snapshot-integrity feature F06.
use super::snapshot_integrity_support::{qualify,manifest,SnapshotIntegrityCard7,SnapshotIntegrityRequest4,SnapshotIntegrityError};
const FEATURE_ID:&str="AFA-stewardship-P32-F06";const CONTRACT_VERSION:&str="stewardship-multimodal-snapshot-integrity-contract_model/1.0";
pub fn stewardship_multimodal_snapshot_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract_model")}
pub fn qualify_stewardship_multimodal_snapshot_integrity_contract_model(request:&SnapshotIntegrityRequest4)->Result<SnapshotIntegrityCard7,SnapshotIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract_model")}
