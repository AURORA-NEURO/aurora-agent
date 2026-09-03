//! Standards P32 federated continual contract-model migration-integrity feature.
use super::migration_integrity_support::{manifest,qualify,MigrationIntegrityCard7,MigrationIntegrityError,MigrationIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-standards-P32-F14";pub const CONTRACT_VERSION:&str="standards-federated_continual_migration_integrity_contract_model/1.0";
pub fn federated_continual_migration_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual","contract-model")}
pub fn qualify_federated_continual_migration_integrity_contract_model(request:&MigrationIntegrityRequest4)->Result<MigrationIntegrityCard7,MigrationIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"federated continual","contract-model")}
