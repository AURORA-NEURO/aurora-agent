//! Sweep P32 local contract_model audit-integrity feature.
use super::audit_integrity_support::{manifest,qualify,AuditCard7,AuditIntegrityError,AuditRequest4};
pub const FEATURE_ID:&str="AFA-sweep-P32-F05";pub const CONTRACT_VERSION:&str="sweep-local_audit_integrity_contract_model/1.0";
pub fn local_audit_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
pub fn qualify_local_audit_integrity_contract_model(request:&AuditRequest4)->Result<AuditCard7,AuditIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
