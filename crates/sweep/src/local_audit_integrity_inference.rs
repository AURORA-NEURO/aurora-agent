//! Sweep P32 local inference audit-integrity feature.
use super::audit_integrity_support::{manifest,qualify,AuditCard7,AuditIntegrityError,AuditRequest4};
pub const FEATURE_ID:&str="AFA-sweep-P32-F01";pub const CONTRACT_VERSION:&str="sweep-local_audit_integrity_inference/1.0";
pub fn local_audit_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","inference")}
pub fn qualify_local_audit_integrity_inference(request:&AuditRequest4)->Result<AuditCard7,AuditIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","inference")}
