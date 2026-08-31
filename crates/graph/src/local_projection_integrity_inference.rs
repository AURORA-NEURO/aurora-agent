//! Graph P32 local inference projection-integrity feature.
use super::projection_integrity_support::{manifest,qualify,ProjectionCard7,ProjectionIntegrityError,ProjectionRequest4};
pub const FEATURE_ID:&str="AFA-graph-P32-F01";pub const CONTRACT_VERSION:&str="graph-local_projection_integrity_inference/1.0";
pub fn local_projection_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","inference")}
pub fn qualify_local_projection_integrity_inference(request:&ProjectionRequest4)->Result<ProjectionCard7,ProjectionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","inference")}
