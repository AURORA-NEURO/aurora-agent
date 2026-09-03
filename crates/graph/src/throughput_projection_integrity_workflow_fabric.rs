//! Graph P32 throughput workflow_fabric projection-integrity feature.
use super::projection_integrity_support::{manifest,qualify,ProjectionCard7,ProjectionIntegrityError,ProjectionRequest4};
pub const FEATURE_ID:&str="AFA-graph-P32-F15";pub const CONTRACT_VERSION:&str="graph-throughput_projection_integrity_workflow_fabric/1.0";
pub fn throughput_projection_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","workflow_fabric")}
pub fn qualify_throughput_projection_integrity_workflow_fabric(request:&ProjectionRequest4)->Result<ProjectionCard7,ProjectionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","workflow_fabric")}
