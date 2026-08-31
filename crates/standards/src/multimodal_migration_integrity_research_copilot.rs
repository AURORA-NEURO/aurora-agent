//! Standards P32 multimodal research-copilot migration-integrity feature.
use super::migration_integrity_support::{manifest,qualify,MigrationIntegrityCard7,MigrationIntegrityError,MigrationIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-standards-P32-F07";pub const CONTRACT_VERSION:&str="standards-multimodal_migration_integrity_research_copilot/1.0";
pub fn multimodal_migration_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","research-copilot")}
pub fn qualify_multimodal_migration_integrity_research_copilot(request:&MigrationIntegrityRequest4)->Result<MigrationIntegrityCard7,MigrationIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","research-copilot")}
