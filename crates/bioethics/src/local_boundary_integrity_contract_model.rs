//! Bioethics P32 local contract-model ethical-boundary integrity feature.
use super::boundary_integrity_support::{manifest,qualify,BoundaryIntegrityCard7,BoundaryIntegrityError,BoundaryIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-bioethics-P32-F02";pub const CONTRACT_VERSION:&str="bioethics-local_boundary_integrity_contract_model/1.0";
pub fn local_boundary_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract-model")}
pub fn qualify_local_boundary_integrity_contract_model(request:&BoundaryIntegrityRequest4)->Result<BoundaryIntegrityCard7,BoundaryIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","contract-model")}
