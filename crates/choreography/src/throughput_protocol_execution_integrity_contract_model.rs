//! Choreography P32 throughput contract_model protocol-execution integrity feature.
use super::protocol_execution_integrity_support::{
    execute, manifest, ProtocolExecutionCard7, ProtocolExecutionIntegrityError,
    ProtocolExecutionRequest4,
};
pub const FEATURE_ID: &str = "AFA-choreography-P32-F07";
pub const CONTRACT_VERSION: &str =
    "choreography-throughput_protocol_execution_integrity_contract_model/1.0";
pub fn throughput_protocol_execution_integrity_contract_model_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "throughput", "contract_model")
}
pub fn execute_throughput_protocol_execution_integrity_contract_model(
    request: &ProtocolExecutionRequest4,
) -> Result<ProtocolExecutionCard7, ProtocolExecutionIntegrityError> {
    execute(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "throughput",
        "contract_model",
    )
}
