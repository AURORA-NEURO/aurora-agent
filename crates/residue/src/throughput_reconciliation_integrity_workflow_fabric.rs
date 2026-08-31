//! Residue P32 throughput workflow-fabric reconciliation-integrity feature.
use super::reconciliation_integrity_support::{manifest,qualify,ReconciliationIntegrityCard7,ReconciliationIntegrityError,ReconciliationIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-residue-P32-F12";pub const CONTRACT_VERSION:&str="residue-throughput_reconciliation_integrity_workflow_fabric/1.0";
pub fn throughput_reconciliation_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","workflow-fabric")}
pub fn qualify_throughput_reconciliation_integrity_workflow_fabric(request:&ReconciliationIntegrityRequest4)->Result<ReconciliationIntegrityCard7,ReconciliationIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","workflow-fabric")}
