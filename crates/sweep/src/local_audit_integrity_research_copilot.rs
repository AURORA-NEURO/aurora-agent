//! Sweep P32 local research_copilot audit-integrity feature.
use super::audit_integrity_support::{manifest,qualify,AuditCard7,AuditIntegrityError,AuditRequest4};
pub const FEATURE_ID:&str="AFA-sweep-P32-F09";pub const CONTRACT_VERSION:&str="sweep-local_audit_integrity_research_copilot/1.0";
pub fn local_audit_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
pub fn qualify_local_audit_integrity_research_copilot(request:&AuditRequest4)->Result<AuditCard7,AuditIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
