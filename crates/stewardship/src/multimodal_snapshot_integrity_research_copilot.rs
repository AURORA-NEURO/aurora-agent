//! Stewardship P32 multimodal multi-study research_copilot snapshot-integrity feature F07.
use super::snapshot_integrity_support::{qualify,manifest,SnapshotIntegrityCard7,SnapshotIntegrityRequest4,SnapshotIntegrityError};
const FEATURE_ID:&str="AFA-stewardship-P32-F07";const CONTRACT_VERSION:&str="stewardship-multimodal-snapshot-integrity-research_copilot/1.0";
pub fn stewardship_multimodal_snapshot_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research_copilot")}
pub fn qualify_stewardship_multimodal_snapshot_integrity_research_copilot(request:&SnapshotIntegrityRequest4)->Result<SnapshotIntegrityCard7,SnapshotIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research_copilot")}
