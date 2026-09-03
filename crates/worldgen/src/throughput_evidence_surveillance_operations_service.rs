//! Prospective high-throughput operations/federation capability (`AFA-worldgen-P01-F31`).
use super::operations_support::{self};
pub use super::operations_support::{OperationsReceipt, OperationsRequest};
pub const FEATURE_ID: &str = "AFA-worldgen-P01-F31";
pub const CONTRACT_VERSION: &str = "worldgen-throughput-evidence-surveillance-operations/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet8@1";
pub const SCALE: &str = "prospective high-throughput";
pub fn worldgen_throughput_evidence_surveillance_operations_manifest() -> serde_json::Value {
    operations_support::manifest(
        FEATURE_ID,
        CONTRACT_VERSION,
        INPUT_SCHEMA,
        OUTPUT_SCHEMA,
        SCALE,
        "A2",
    )
}
pub fn operate_worldgen_throughput_evidence_surveillance(
    request: &OperationsRequest,
) -> Result<OperationsReceipt, operations_support::OperationsError> {
    operations_support::operate(request, FEATURE_ID, CONTRACT_VERSION)
}
pub use operations_support::{OperationsDisposition, OperationsError, OperationsEvent};
#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_ids::ContentHash;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn r() -> OperationsRequest {
        OperationsRequest {
            request_id: "request:throughput".into(),
            operator: "preclinical neuroscientist".into(),
            scope: "stream:high-throughput".into(),
            scale: SCALE.into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            events: vec![OperationsEvent {
                event_id: "event:a".into(),
                evidence_state: "supported".into(),
                provenance_digest: h("p"),
                permitted: true,
                retryable: false,
                negative_result: false,
            }],
            capacity: 8,
            budget_units: 8,
            requested_units: 1,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: operations_support::BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified() {
        assert_eq!(
            operate_worldgen_throughput_evidence_surveillance(&r())
                .unwrap()
                .disposition,
            OperationsDisposition::Qualified
        );
    }
}
