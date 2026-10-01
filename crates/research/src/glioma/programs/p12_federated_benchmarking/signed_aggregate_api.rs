//! Idempotent signed aggregate-result submission for the glioma federation.
//!
//! The submission boundary binds an aggregate result to a benchmark, schema, signer, calibration,
//! provenance, privacy accounting, and local approval before it can be consumed by integrity or
//! quorum gates. It is intentionally an institution-local protocol simulator: no signature keys,
//! credentials, raw experimental data, or clinical decisions cross the MCP boundary.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaSignedAggregateContribution1@1";
pub const API_VERSION: &str = "aggregate-result-api/1.0";
pub const MAX_TEXT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateSubmissionStatus {
    Accepted,
    Duplicate,
    NeedsApproval,
    Revoked,
    Overspend,
    Rejected,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateSubmissionReason {
    InvalidApiVersion,
    MissingIdentity,
    PolicyScopeMismatch,
    SchemaMismatch,
    BenchmarkMismatch,
    MissingSignature,
    InvalidSignature,
    ApprovalMissing,
    RevokedSite,
    PrivacyOverspend,
    MissingCalibration,
    MissingProvenance,
    ProtectedData,
    RawDataNotLocal,
    NonAggregatePayload,
    DuplicateSubmission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedAggregateSubmissionRequest {
    pub api_version: String,
    pub submission_id: String,
    pub idempotency_key: String,
    pub site_id: String,
    pub benchmark_id: String,
    pub required_benchmark_id: String,
    pub schema_version: String,
    pub required_schema_version: String,
    pub policy_scope: String,
    pub required_policy_scope: String,
    pub aggregate_digest: ContentHash,
    pub signature_digest: Option<ContentHash>,
    pub signer_id: Option<String>,
    pub signature_valid: bool,
    pub calibration_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub local_approval: bool,
    pub revoked: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub privacy_cost_milli: u64,
    pub privacy_budget_remaining_milli: u64,
    pub replay_of_submission_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateSubmissionReceipt {
    pub receipt_id: String,
    pub submission_digest: ContentHash,
    pub submission_id: String,
    pub site_id: String,
    pub status: AggregateSubmissionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedAggregateContribution {
    pub feature_id: String,
    pub output_schema: String,
    pub api_version: String,
    pub submission_id: String,
    pub idempotency_key: String,
    pub site_id: String,
    pub benchmark_id: String,
    pub aggregate_digest: ContentHash,
    pub status: AggregateSubmissionStatus,
    pub reason_order: Vec<AggregateSubmissionReason>,
    pub receipt: Option<AggregateSubmissionReceipt>,
    pub aggregate_consumption_permitted: bool,
    pub raw_data_moved: bool,
    pub credentials_exchanged: bool,
    pub clinical_decision_made: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignedAggregateApiError {
    #[error("signed aggregate request is invalid: {0}")]
    InvalidRequest(String),
    #[error("signed aggregate output is invalid: {0}")]
    InvalidOutput(String),
    #[error("signed aggregate digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT && !value.chars().any(char::is_control)
}

fn digest_body(output: &SignedAggregateContribution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "api_version": output.api_version,
        "submission_id": output.submission_id,
        "idempotency_key": output.idempotency_key,
        "site_id": output.site_id,
        "benchmark_id": output.benchmark_id,
        "aggregate_digest": output.aggregate_digest,
        "status": output.status,
        "reason_order": output.reason_order,
        "receipt": output.receipt,
        "aggregate_consumption_permitted": output.aggregate_consumption_permitted,
        "raw_data_moved": output.raw_data_moved,
        "credentials_exchanged": output.credentials_exchanged,
        "clinical_decision_made": output.clinical_decision_made,
    })
}

fn request_digest(
    request: &SignedAggregateSubmissionRequest,
) -> Result<ContentHash, SignedAggregateApiError> {
    ContentHash::of_value(&serde_json::json!({
        "api_version": request.api_version.clone(),
        "submission_id": request.submission_id.clone(),
        "idempotency_key": request.idempotency_key.clone(),
        "site_id": request.site_id.clone(),
        "benchmark_id": request.benchmark_id.clone(),
        "schema_version": request.schema_version.clone(),
        "policy_scope": request.policy_scope.clone(),
        "aggregate_digest": request.aggregate_digest.clone(),
        "signature_digest": request.signature_digest.clone(),
        "signer_id": request.signer_id.clone(),
        "calibration_digest": request.calibration_digest.clone(),
        "provenance_digest": request.provenance_digest.clone(),
        "local_approval": request.local_approval,
        "revoked": request.revoked,
        "aggregate_only": request.aggregate_only,
        "raw_data_local": request.raw_data_local,
        "privacy_cost_milli": request.privacy_cost_milli,
        "privacy_budget_remaining_milli": request.privacy_budget_remaining_milli,
        "replay_of_submission_digest": request.replay_of_submission_digest.clone(),
    }))
    .map_err(|error| SignedAggregateApiError::Digest(error.to_string()))
}

fn validate_request(
    request: &SignedAggregateSubmissionRequest,
) -> Result<(), SignedAggregateApiError> {
    if request.api_version != API_VERSION
        || !safe_text(&request.submission_id)
        || !safe_text(&request.idempotency_key)
        || !safe_text(&request.site_id)
        || !safe_text(&request.benchmark_id)
        || !safe_text(&request.required_benchmark_id)
        || !safe_text(&request.schema_version)
        || !safe_text(&request.required_schema_version)
        || !safe_text(&request.policy_scope)
        || !safe_text(&request.required_policy_scope)
        || request.aggregate_digest.as_str().len() != 64
    {
        return Err(SignedAggregateApiError::InvalidRequest(
            "API version, bounded identity, policy/schema/benchmark bindings, and aggregate digest are required".into(),
        ));
    }
    Ok(())
}

impl SignedAggregateContribution {
    pub fn validate(&self) -> Result<(), SignedAggregateApiError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.api_version != API_VERSION
            || !safe_text(&self.submission_id)
            || !safe_text(&self.idempotency_key)
            || !safe_text(&self.site_id)
            || !safe_text(&self.benchmark_id)
            || self.aggregate_digest.as_str().len() != 64
            || self.raw_data_moved
            || self.credentials_exchanged
            || self.clinical_decision_made
            || self.digest.as_str().len() != 64
        {
            return Err(SignedAggregateApiError::InvalidOutput(
                "signed aggregate identity, boundary, or digest invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| SignedAggregateApiError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SignedAggregateApiError::InvalidOutput(
                "signed aggregate digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Validate and admit one signed aggregate contribution at a site-local API boundary.
pub fn submit_glioma_signed_aggregate(
    request: &SignedAggregateSubmissionRequest,
) -> Result<SignedAggregateContribution, SignedAggregateApiError> {
    validate_request(request)?;
    let mut reasons = Vec::new();
    let status = if request.revoked {
        reasons.push(AggregateSubmissionReason::RevokedSite);
        AggregateSubmissionStatus::Revoked
    } else if request.replay_of_submission_digest.is_some() {
        reasons.push(AggregateSubmissionReason::DuplicateSubmission);
        AggregateSubmissionStatus::Duplicate
    } else if request.benchmark_id != request.required_benchmark_id {
        reasons.push(AggregateSubmissionReason::BenchmarkMismatch);
        AggregateSubmissionStatus::Rejected
    } else if request.schema_version != request.required_schema_version {
        reasons.push(AggregateSubmissionReason::SchemaMismatch);
        AggregateSubmissionStatus::Rejected
    } else if request.policy_scope != request.required_policy_scope {
        reasons.push(AggregateSubmissionReason::PolicyScopeMismatch);
        AggregateSubmissionStatus::Rejected
    } else if request.signer_id.as_deref().is_none_or(|id| !safe_text(id))
        || request.signature_digest.is_none()
    {
        reasons.push(AggregateSubmissionReason::MissingSignature);
        AggregateSubmissionStatus::Rejected
    } else if !request.signature_valid {
        reasons.push(AggregateSubmissionReason::InvalidSignature);
        AggregateSubmissionStatus::Rejected
    } else if !request.local_approval {
        reasons.push(AggregateSubmissionReason::ApprovalMissing);
        AggregateSubmissionStatus::NeedsApproval
    } else if request.privacy_cost_milli > request.privacy_budget_remaining_milli {
        reasons.push(AggregateSubmissionReason::PrivacyOverspend);
        AggregateSubmissionStatus::Overspend
    } else if request.calibration_digest.is_none() {
        reasons.push(AggregateSubmissionReason::MissingCalibration);
        AggregateSubmissionStatus::Unresolved
    } else if request.provenance_digest.is_none() {
        reasons.push(AggregateSubmissionReason::MissingProvenance);
        AggregateSubmissionStatus::Unresolved
    } else if !request.aggregate_only {
        reasons.push(AggregateSubmissionReason::NonAggregatePayload);
        AggregateSubmissionStatus::Rejected
    } else if !request.raw_data_local {
        reasons.push(AggregateSubmissionReason::RawDataNotLocal);
        AggregateSubmissionStatus::Rejected
    } else if request.contains_human_data || request.contains_direct_identifiers {
        reasons.push(AggregateSubmissionReason::ProtectedData);
        AggregateSubmissionStatus::Rejected
    } else {
        AggregateSubmissionStatus::Accepted
    };
    let submission_digest = request_digest(request)?;
    let receipt = if status == AggregateSubmissionStatus::Accepted {
        Some(AggregateSubmissionReceipt {
            receipt_id: format!("aggregate-receipt-{}", &submission_digest.as_str()[..16]),
            submission_digest: submission_digest.clone(),
            submission_id: request.submission_id.clone(),
            site_id: request.site_id.clone(),
            status,
        })
    } else {
        None
    };
    let mut output = SignedAggregateContribution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        api_version: API_VERSION.into(),
        submission_id: request.submission_id.clone(),
        idempotency_key: request.idempotency_key.clone(),
        site_id: request.site_id.clone(),
        benchmark_id: request.benchmark_id.clone(),
        aggregate_digest: request.aggregate_digest.clone(),
        status,
        reason_order: reasons,
        receipt,
        aggregate_consumption_permitted: status == AggregateSubmissionStatus::Accepted,
        raw_data_moved: false,
        credentials_exchanged: false,
        clinical_decision_made: false,
        digest: ContentHash::of_bytes(b"unsealed-glioma-signed-aggregate"),
    };
    output.digest = ContentHash::of_value(&digest_body(&output))
        .map_err(|error| SignedAggregateApiError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> SignedAggregateSubmissionRequest {
        SignedAggregateSubmissionRequest {
            api_version: API_VERSION.into(),
            submission_id: "submission-1".into(),
            idempotency_key: "idem-1".into(),
            site_id: "site-a".into(),
            benchmark_id: "benchmark-1".into(),
            required_benchmark_id: "benchmark-1".into(),
            schema_version: "aggregate-v1".into(),
            required_schema_version: "aggregate-v1".into(),
            policy_scope: "glioma-v1".into(),
            required_policy_scope: "glioma-v1".into(),
            aggregate_digest: digest("aggregate"),
            signature_digest: Some(digest("signature")),
            signer_id: Some("signer-a".into()),
            signature_valid: true,
            calibration_digest: Some(digest("calibration")),
            provenance_digest: Some(digest("provenance")),
            local_approval: true,
            revoked: false,
            aggregate_only: true,
            raw_data_local: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            privacy_cost_milli: 25,
            privacy_budget_remaining_milli: 100,
            replay_of_submission_digest: None,
        }
    }

    #[test]
    fn valid_signed_aggregate_is_accepted_with_receipt() {
        let output = submit_glioma_signed_aggregate(&request()).unwrap();
        assert_eq!(output.status, AggregateSubmissionStatus::Accepted);
        assert!(output.aggregate_consumption_permitted);
        assert!(output.receipt.is_some());
    }

    #[test]
    fn schema_signature_and_privacy_failures_are_explicit() {
        let mut schema = request();
        schema.schema_version = "old".into();
        assert_eq!(
            submit_glioma_signed_aggregate(&schema).unwrap().status,
            AggregateSubmissionStatus::Rejected
        );
        let mut signature = request();
        signature.signature_valid = false;
        assert_eq!(
            submit_glioma_signed_aggregate(&signature).unwrap().status,
            AggregateSubmissionStatus::Rejected
        );
        let mut spend = request();
        spend.privacy_budget_remaining_milli = 10;
        assert_eq!(
            submit_glioma_signed_aggregate(&spend).unwrap().status,
            AggregateSubmissionStatus::Overspend
        );
    }

    #[test]
    fn missing_approval_calibration_and_provenance_do_not_admit() {
        let mut approval = request();
        approval.local_approval = false;
        assert_eq!(
            submit_glioma_signed_aggregate(&approval).unwrap().status,
            AggregateSubmissionStatus::NeedsApproval
        );
        let mut calibration = request();
        calibration.calibration_digest = None;
        assert_eq!(
            submit_glioma_signed_aggregate(&calibration).unwrap().status,
            AggregateSubmissionStatus::Unresolved
        );
        let mut provenance = request();
        provenance.provenance_digest = None;
        assert_eq!(
            submit_glioma_signed_aggregate(&provenance).unwrap().status,
            AggregateSubmissionStatus::Unresolved
        );
    }

    #[test]
    fn duplicate_revoked_and_protected_submissions_are_blocked() {
        let mut duplicate = request();
        duplicate.replay_of_submission_digest = Some(digest("prior"));
        assert_eq!(
            submit_glioma_signed_aggregate(&duplicate).unwrap().status,
            AggregateSubmissionStatus::Duplicate
        );
        let mut revoked = request();
        revoked.revoked = true;
        assert_eq!(
            submit_glioma_signed_aggregate(&revoked).unwrap().status,
            AggregateSubmissionStatus::Revoked
        );
        let mut protected = request();
        protected.contains_direct_identifiers = true;
        assert_eq!(
            submit_glioma_signed_aggregate(&protected).unwrap().status,
            AggregateSubmissionStatus::Rejected
        );
    }

    #[test]
    fn submission_replay_is_content_deterministic() {
        let first = submit_glioma_signed_aggregate(&request()).unwrap();
        let second = submit_glioma_signed_aggregate(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.digest, second.digest);
    }
}
