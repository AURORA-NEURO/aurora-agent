//! Choreography P32 federated_continual workflow_fabric protocol-execution integrity feature.
use super::protocol_execution_integrity_support::{
    execute, manifest, ProtocolExecutionCard7, ProtocolExecutionIntegrityError,
    ProtocolExecutionRequest4,
};
pub const FEATURE_ID: &str = "AFA-choreography-P32-F16";
pub const CONTRACT_VERSION: &str =
    "choreography-federated_continual_protocol_execution_integrity_workflow_fabric/1.0";
pub fn federated_continual_protocol_execution_integrity_workflow_fabric_manifest(
) -> serde_json::Value {
    manifest(
        FEATURE_ID,
        CONTRACT_VERSION,
        "federated_continual",
        "workflow_fabric",
    )
}
pub fn execute_federated_continual_protocol_execution_integrity_workflow_fabric(
    request: &ProtocolExecutionRequest4,
) -> Result<ProtocolExecutionCard7, ProtocolExecutionIntegrityError> {
    execute(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "federated_continual",
        "workflow_fabric",
    )
}
