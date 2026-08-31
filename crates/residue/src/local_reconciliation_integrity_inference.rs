//! Residue P32 local inference reconciliation-integrity feature.
use super::reconciliation_integrity_support::{manifest,qualify,ReconciliationIntegrityCard7,ReconciliationIntegrityError,ReconciliationIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-residue-P32-F01";pub const CONTRACT_VERSION:&str="residue-local_reconciliation_integrity_inference/1.0";
pub fn local_reconciliation_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","inference")}
pub fn qualify_local_reconciliation_integrity_inference(request:&ReconciliationIntegrityRequest4)->Result<ReconciliationIntegrityCard7,ReconciliationIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","inference")}
