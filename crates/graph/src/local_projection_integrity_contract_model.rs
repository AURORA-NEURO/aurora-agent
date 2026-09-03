//! Graph P32 local contract_model projection-integrity feature.
use super::projection_integrity_support::{manifest,qualify,ProjectionCard7,ProjectionIntegrityError,ProjectionRequest4};
pub const FEATURE_ID:&str="AFA-graph-P32-F05";pub const CONTRACT_VERSION:&str="graph-local_projection_integrity_contract_model/1.0";
pub fn local_projection_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
pub fn qualify_local_projection_integrity_contract_model(request:&ProjectionRequest4)->Result<ProjectionCard7,ProjectionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
