//! Choreography P32 local workflow_fabric protocol-execution integrity feature.
use super::protocol_execution_integrity_support::{
    execute, manifest, ProtocolExecutionCard7, ProtocolExecutionIntegrityError,
    ProtocolExecutionRequest4,
};
pub const FEATURE_ID: &str = "AFA-choreography-P32-F13";
pub const CONTRACT_VERSION: &str =
    "choreography-local_protocol_execution_integrity_workflow_fabric/1.0";
pub fn local_protocol_execution_integrity_workflow_fabric_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "local", "workflow_fabric")
}
pub fn execute_local_protocol_execution_integrity_workflow_fabric(
    request: &ProtocolExecutionRequest4,
) -> Result<ProtocolExecutionCard7, ProtocolExecutionIntegrityError> {
    execute(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "local",
        "workflow_fabric",
    )
}
