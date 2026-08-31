//! Stewardship P32 multimodal multi-study workflow_fabric snapshot-integrity feature F08.
use super::snapshot_integrity_support::{qualify,manifest,SnapshotIntegrityCard7,SnapshotIntegrityRequest4,SnapshotIntegrityError};
const FEATURE_ID:&str="AFA-stewardship-P32-F08";const CONTRACT_VERSION:&str="stewardship-multimodal-snapshot-integrity-workflow_fabric/1.0";
pub fn stewardship_multimodal_snapshot_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
pub fn qualify_stewardship_multimodal_snapshot_integrity_workflow_fabric(request:&SnapshotIntegrityRequest4)->Result<SnapshotIntegrityCard7,SnapshotIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
