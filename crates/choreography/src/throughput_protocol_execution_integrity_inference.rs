//! Choreography P32 throughput inference protocol-execution integrity feature.
use super::protocol_execution_integrity_support::{
    execute, manifest, ProtocolExecutionCard7, ProtocolExecutionIntegrityError,
    ProtocolExecutionRequest4,
};
pub const FEATURE_ID: &str = "AFA-choreography-P32-F03";
pub const CONTRACT_VERSION: &str =
    "choreography-throughput_protocol_execution_integrity_inference/1.0";
pub fn throughput_protocol_execution_integrity_inference_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "throughput", "inference")
}
pub fn execute_throughput_protocol_execution_integrity_inference(
    request: &ProtocolExecutionRequest4,
) -> Result<ProtocolExecutionCard7, ProtocolExecutionIntegrityError> {
    execute(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "throughput",
        "inference",
    )
}
