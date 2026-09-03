//! Bioethics P32 local workflow-fabric ethical-boundary integrity feature.
use super::boundary_integrity_support::{manifest,qualify,BoundaryIntegrityCard7,BoundaryIntegrityError,BoundaryIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-bioethics-P32-F04";pub const CONTRACT_VERSION:&str="bioethics-local_boundary_integrity_workflow_fabric/1.0";
pub fn local_boundary_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","workflow-fabric")}
pub fn qualify_local_boundary_integrity_workflow_fabric(request:&BoundaryIntegrityRequest4)->Result<BoundaryIntegrityCard7,BoundaryIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","workflow-fabric")}
