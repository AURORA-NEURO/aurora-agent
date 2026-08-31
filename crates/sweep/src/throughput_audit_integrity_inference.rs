//! Sweep P32 throughput inference audit-integrity feature.
use super::audit_integrity_support::{manifest,qualify,AuditCard7,AuditIntegrityError,AuditRequest4};
pub const FEATURE_ID:&str="AFA-sweep-P32-F03";pub const CONTRACT_VERSION:&str="sweep-throughput_audit_integrity_inference/1.0";
pub fn throughput_audit_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
pub fn qualify_throughput_audit_integrity_inference(request:&AuditRequest4)->Result<AuditCard7,AuditIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
