//! Standards P32 local contract-model migration-integrity feature.
use super::migration_integrity_support::{manifest,qualify,MigrationIntegrityCard7,MigrationIntegrityError,MigrationIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-standards-P32-F02";pub const CONTRACT_VERSION:&str="standards-local_migration_integrity_contract_model/1.0";
pub fn local_migration_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract-model")}
pub fn qualify_local_migration_integrity_contract_model(request:&MigrationIntegrityRequest4)->Result<MigrationIntegrityCard7,MigrationIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","contract-model")}
