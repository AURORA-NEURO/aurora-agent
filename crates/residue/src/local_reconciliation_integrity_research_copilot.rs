//! Residue P32 local research-copilot reconciliation-integrity feature.
use super::reconciliation_integrity_support::{manifest,qualify,ReconciliationIntegrityCard7,ReconciliationIntegrityError,ReconciliationIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-residue-P32-F03";pub const CONTRACT_VERSION:&str="residue-local_reconciliation_integrity_research_copilot/1.0";
pub fn local_reconciliation_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","research-copilot")}
pub fn qualify_local_reconciliation_integrity_research_copilot(request:&ReconciliationIntegrityRequest4)->Result<ReconciliationIntegrityCard7,ReconciliationIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","research-copilot")}
