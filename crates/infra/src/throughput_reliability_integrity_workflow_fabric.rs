//! Infra P32 throughput workflow-fabric reliability-integrity feature.
use super::reliability_integrity_support::{manifest,qualify,ReliabilityIntegrityCard7,ReliabilityIntegrityError,ReliabilityIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-infra-P32-F12";pub const CONTRACT_VERSION:&str="infra-throughput_reliability_integrity_workflow_fabric/1.0";
pub fn throughput_reliability_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","workflow-fabric")}
pub fn qualify_throughput_reliability_integrity_workflow_fabric(request:&ReliabilityIntegrityRequest4)->Result<ReliabilityIntegrityCard7,ReliabilityIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","workflow-fabric")}
