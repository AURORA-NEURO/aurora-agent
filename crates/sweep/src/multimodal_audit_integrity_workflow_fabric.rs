//! Sweep P32 multimodal workflow_fabric audit-integrity feature.
use super::audit_integrity_support::{manifest,qualify,AuditCard7,AuditIntegrityError,AuditRequest4};
pub const FEATURE_ID:&str="AFA-sweep-P32-F14";pub const CONTRACT_VERSION:&str="sweep-multimodal_audit_integrity_workflow_fabric/1.0";
pub fn multimodal_audit_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","workflow_fabric")}
pub fn qualify_multimodal_audit_integrity_workflow_fabric(request:&AuditRequest4)->Result<AuditCard7,AuditIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","workflow_fabric")}
