//! Standards P32 throughput inference migration-integrity feature.
use super::migration_integrity_support::{manifest,qualify,MigrationIntegrityCard7,MigrationIntegrityError,MigrationIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-standards-P32-F09";pub const CONTRACT_VERSION:&str="standards-throughput_migration_integrity_inference/1.0";
pub fn throughput_migration_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
pub fn qualify_throughput_migration_integrity_inference(request:&MigrationIntegrityRequest4)->Result<MigrationIntegrityCard7,MigrationIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
