//! Choreography P32 multimodal workflow_fabric protocol-execution integrity feature.
use super::protocol_execution_integrity_support::{
    execute, manifest, ProtocolExecutionCard7, ProtocolExecutionIntegrityError,
    ProtocolExecutionRequest4,
};
pub const FEATURE_ID: &str = "AFA-choreography-P32-F14";
pub const CONTRACT_VERSION: &str =
    "choreography-multimodal_protocol_execution_integrity_workflow_fabric/1.0";
pub fn multimodal_protocol_execution_integrity_workflow_fabric_manifest() -> serde_json::Value {
    manifest(
        FEATURE_ID,
        CONTRACT_VERSION,
        "multimodal",
        "workflow_fabric",
    )
}
pub fn execute_multimodal_protocol_execution_integrity_workflow_fabric(
    request: &ProtocolExecutionRequest4,
) -> Result<ProtocolExecutionCard7, ProtocolExecutionIntegrityError> {
    execute(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "multimodal",
        "workflow_fabric",
    )
}
