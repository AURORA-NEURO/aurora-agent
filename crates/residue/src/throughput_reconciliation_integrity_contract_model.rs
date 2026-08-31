//! Residue P32 throughput contract-model reconciliation-integrity feature.
use super::reconciliation_integrity_support::{manifest,qualify,ReconciliationIntegrityCard7,ReconciliationIntegrityError,ReconciliationIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-residue-P32-F10";pub const CONTRACT_VERSION:&str="residue-throughput_reconciliation_integrity_contract_model/1.0";
pub fn throughput_reconciliation_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","contract-model")}
pub fn qualify_throughput_reconciliation_integrity_contract_model(request:&ReconciliationIntegrityRequest4)->Result<ReconciliationIntegrityCard7,ReconciliationIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","contract-model")}
