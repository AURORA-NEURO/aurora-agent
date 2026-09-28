//! Purpose-bound, revocable access control for federated glioma decision context.
//!
//! The governor grants only named context fields to a verified preclinical purpose. Membership,
//! policy, scope, locality, expiration, and revocation are checked before a grant is emitted;
//! denied fields are returned as metadata-only redaction reasons. No context value or protected
//! payload crosses this contract.

use super::partition_checkpoint::ContextCheckpointField;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedContextAccessDecision1@1";
pub const MAX_FIELDS: usize = 32;
pub const MAX_REVOKED_CAPABILITIES: usize = 256;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextAccessPurpose {
    ResearchPlanning,
    QualityControl,
    Replication,
    AggregateBenchmarking,
    PublicationPreparation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextAccessDecision {
    Allow,
    Redact,
    Deny,
    Revoked,
    ApprovalRequired,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextAccessRequest {
    pub requester_id: String,
    pub site_id: String,
    pub study_id: String,
    pub capability_id: String,
    pub purpose: ContextAccessPurpose,
    pub requested_field_order: Vec<ContextCheckpointField>,
    pub membership_digest: ContentHash,
    pub policy_digest: ContentHash,
    pub scope_digest: ContentHash,
    pub policy_epoch: u32,
    pub current_tick: u64,
    pub grant_ttl_ticks: u64,
    pub membership_active: bool,
    pub policy_allowed: bool,
    pub approval_granted: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub revoked_capability_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAccessFieldDecision {
    pub field: ContextCheckpointField,
    pub permitted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextAccessDecision {
    pub feature_id: String,
    pub output_schema: String,
    pub requester_id: String,
    pub site_id: String,
    pub study_id: String,
    pub capability_id: String,
    pub purpose: ContextAccessPurpose,
    pub decision: ContextAccessDecision,
    pub granted_field_order: Vec<ContextCheckpointField>,
    pub redacted_field_order: Vec<ContextCheckpointField>,
    pub field_decisions: Vec<ContextAccessFieldDecision>,
    pub issued_tick: u64,
    pub expires_tick: Option<u64>,
    pub revocation_checked: bool,
    pub local_only: bool,
    pub approval_required: bool,
    pub reasons: Vec<String>,
    pub grant_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextAccessGovernorError {
    #[error("context access request is invalid: {0}")]
    InvalidRequest(String),
    #[error("context access decision is invalid: {0}")]
    InvalidOutput(String),
    #[error("context access digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn body(output: &FederatedContextAccessDecision) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "requester_id": output.requester_id,
        "site_id": output.site_id,
        "study_id": output.study_id,
        "capability_id": output.capability_id,
        "purpose": output.purpose,
        "decision": output.decision,
        "granted_field_order": output.granted_field_order,
        "redacted_field_order": output.redacted_field_order,
        "field_decisions": output.field_decisions,
        "issued_tick": output.issued_tick,
        "expires_tick": output.expires_tick,
        "revocation_checked": output.revocation_checked,
        "local_only": output.local_only,
        "approval_required": output.approval_required,
        "reasons": output.reasons,
    })
}

fn field_allowed(purpose: ContextAccessPurpose, field: ContextCheckpointField) -> bool {
    match purpose {
        ContextAccessPurpose::ResearchPlanning => true,
        ContextAccessPurpose::QualityControl => matches!(
            field,
            ContextCheckpointField::Objective
                | ContextCheckpointField::ClaimOrder
                | ContextCheckpointField::OmissionOrder
                | ContextCheckpointField::NegativeEvidenceOrder
                | ContextCheckpointField::UncertaintyOrder
                | ContextCheckpointField::Disposition
        ),
        ContextAccessPurpose::Replication => !matches!(field, ContextCheckpointField::ActionOrder),
        ContextAccessPurpose::AggregateBenchmarking => matches!(
            field,
            ContextCheckpointField::Objective
                | ContextCheckpointField::ClaimOrder
                | ContextCheckpointField::OmissionOrder
                | ContextCheckpointField::NegativeEvidenceOrder
                | ContextCheckpointField::UncertaintyOrder
                | ContextCheckpointField::Disposition
        ),
        ContextAccessPurpose::PublicationPreparation => matches!(
            field,
            ContextCheckpointField::Objective
                | ContextCheckpointField::ClaimOrder
                | ContextCheckpointField::OmissionOrder
                | ContextCheckpointField::NegativeEvidenceOrder
                | ContextCheckpointField::UncertaintyOrder
                | ContextCheckpointField::Disposition
        ),
    }
}

fn validate_request(
    request: &FederatedContextAccessRequest,
) -> Result<(), ContextAccessGovernorError> {
    if !safe_text(&request.requester_id)
        || !safe_text(&request.site_id)
        || !safe_text(&request.study_id)
        || !safe_text(&request.capability_id)
        || request.requested_field_order.is_empty()
        || request.requested_field_order.len() > MAX_FIELDS
        || !canonical(&request.requested_field_order)
        || request.policy_epoch == 0
        || request.current_tick == 0
        || request.grant_ttl_ticks == 0
        || request
            .current_tick
            .checked_add(request.grant_ttl_ticks)
            .is_none()
        || !valid_hash(&request.membership_digest)
        || !valid_hash(&request.policy_digest)
        || !valid_hash(&request.scope_digest)
        || request.revoked_capability_order.len() > MAX_REVOKED_CAPABILITIES
        || !canonical(&request.revoked_capability_order)
        || request
            .revoked_capability_order
            .iter()
            .any(|id| !safe_text(id))
        || !request.local_only
        || request.contains_human_data
        || request.contains_direct_identifiers
    {
        return Err(ContextAccessGovernorError::InvalidRequest(
            "identity, purpose, sorted fields, bounded expiry, policy hashes, locality, and protected-data exclusions are required".into(),
        ));
    }
    Ok(())
}

impl FederatedContextAccessDecision {
    pub fn validate(&self) -> Result<(), ContextAccessGovernorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.requester_id)
            || !safe_text(&self.site_id)
            || !safe_text(&self.study_id)
            || !safe_text(&self.capability_id)
            || !canonical(&self.granted_field_order)
            || !canonical(&self.redacted_field_order)
            || self
                .granted_field_order
                .iter()
                .any(|field| self.redacted_field_order.contains(field))
            || self
                .field_decisions
                .iter()
                .any(|field| !safe_text(&field.reason))
            || !self.revocation_checked
            || self.issued_tick == 0
            || self
                .expires_tick
                .is_some_and(|tick| tick <= self.issued_tick)
            || !self.local_only
            || self.grant_digest.as_str().len() != 64
        {
            return Err(ContextAccessGovernorError::InvalidOutput(
                "access identity, field partition, revocation, expiry, locality, or digest invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&body(self))
            .map_err(|error| ContextAccessGovernorError::Digest(error.to_string()))?;
        if expected != self.grant_digest {
            return Err(ContextAccessGovernorError::InvalidOutput(
                "access grant digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Decide a least-privilege, purpose-bound field grant for local federated research context.
pub fn govern_federated_context_access(
    request: &FederatedContextAccessRequest,
) -> Result<FederatedContextAccessDecision, ContextAccessGovernorError> {
    validate_request(request)?;
    let capability_revoked = request
        .revoked_capability_order
        .binary_search(&request.capability_id)
        .is_ok();
    let mut reasons = Vec::new();
    let mut granted = Vec::new();
    let mut redacted = Vec::new();
    let mut field_decisions = Vec::new();
    for field in &request.requested_field_order {
        let permitted = field_allowed(request.purpose, *field);
        field_decisions.push(ContextAccessFieldDecision {
            field: *field,
            permitted,
            reason: if permitted {
                "purpose permits this typed context field".into()
            } else {
                "purpose does not permit this field; payload remains local".into()
            },
        });
        if permitted {
            granted.push(*field);
        } else {
            redacted.push(*field);
        }
    }
    let approval_required = !request.approval_granted;
    let (decision, expires_tick) = if capability_revoked {
        reasons.push("capability is present in the revocation set".into());
        (ContextAccessDecision::Revoked, None)
    } else if !request.membership_active {
        reasons.push("membership is inactive or unverified".into());
        (ContextAccessDecision::Deny, None)
    } else if !request.policy_allowed {
        reasons.push("purpose or policy does not authorize this request".into());
        (ContextAccessDecision::Deny, None)
    } else if approval_required {
        reasons.push("explicit steward approval is required before a grant can be used".into());
        (ContextAccessDecision::ApprovalRequired, None)
    } else if granted.is_empty() {
        reasons.push("no requested field is permitted for the declared purpose".into());
        (ContextAccessDecision::Deny, None)
    } else if !redacted.is_empty() {
        reasons.push("least-privilege redaction removed one or more requested fields".into());
        (
            ContextAccessDecision::Redact,
            Some(request.current_tick + request.grant_ttl_ticks),
        )
    } else {
        reasons.push(
            "all requested fields satisfy purpose, membership, policy, and approval gates".into(),
        );
        (
            ContextAccessDecision::Allow,
            Some(request.current_tick + request.grant_ttl_ticks),
        )
    };
    let mut output = FederatedContextAccessDecision {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        requester_id: request.requester_id.clone(),
        site_id: request.site_id.clone(),
        study_id: request.study_id.clone(),
        capability_id: request.capability_id.clone(),
        purpose: request.purpose,
        decision,
        granted_field_order: granted,
        redacted_field_order: redacted,
        field_decisions,
        issued_tick: request.current_tick,
        expires_tick,
        revocation_checked: true,
        local_only: true,
        approval_required,
        reasons,
        grant_digest: ContentHash::of_bytes(b"unsealed-glioma-context-access"),
    };
    output.grant_digest = ContentHash::of_value(&body(&output))
        .map_err(|error| ContextAccessGovernorError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> FederatedContextAccessRequest {
        FederatedContextAccessRequest {
            requester_id: "agent-a".into(),
            site_id: "site-a".into(),
            study_id: "study-a".into(),
            capability_id: "context-read-v1".into(),
            purpose: ContextAccessPurpose::ResearchPlanning,
            requested_field_order: vec![
                ContextCheckpointField::Objective,
                ContextCheckpointField::OmissionOrder,
                ContextCheckpointField::Disposition,
            ],
            membership_digest: ContentHash::of_bytes(b"membership"),
            policy_digest: ContentHash::of_bytes(b"policy"),
            scope_digest: ContentHash::of_bytes(b"scope"),
            policy_epoch: 2,
            current_tick: 10,
            grant_ttl_ticks: 20,
            membership_active: true,
            policy_allowed: true,
            approval_granted: true,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            revoked_capability_order: Vec::new(),
        }
    }

    #[test]
    fn grants_all_preclinical_planning_fields_with_expiry() {
        let output = govern_federated_context_access(&request()).unwrap();
        assert_eq!(output.decision, ContextAccessDecision::Allow);
        assert_eq!(output.expires_tick, Some(30));
        assert!(output.validate().is_ok());
    }

    #[test]
    fn purpose_redacts_action_fields_without_denied_payload() {
        let mut req = request();
        req.purpose = ContextAccessPurpose::AggregateBenchmarking;
        req.requested_field_order = vec![
            ContextCheckpointField::ClaimOrder,
            ContextCheckpointField::ActionOrder,
        ];
        let output = govern_federated_context_access(&req).unwrap();
        assert_eq!(output.decision, ContextAccessDecision::Redact);
        assert_eq!(
            output.redacted_field_order,
            vec![ContextCheckpointField::ActionOrder]
        );
    }

    #[test]
    fn revoked_capability_fails_closed() {
        let mut req = request();
        req.revoked_capability_order = vec!["context-read-v1".into()];
        let output = govern_federated_context_access(&req).unwrap();
        assert_eq!(output.decision, ContextAccessDecision::Revoked);
        assert!(output.expires_tick.is_none());
    }

    #[test]
    fn inactive_membership_and_missing_approval_are_explicit() {
        let mut inactive = request();
        inactive.membership_active = false;
        assert_eq!(
            govern_federated_context_access(&inactive).unwrap().decision,
            ContextAccessDecision::Deny
        );
        let mut approval = request();
        approval.approval_granted = false;
        assert_eq!(
            govern_federated_context_access(&approval).unwrap().decision,
            ContextAccessDecision::ApprovalRequired
        );
    }

    #[test]
    fn protected_or_nonlocal_requests_are_rejected_before_granting() {
        let mut protected = request();
        protected.contains_human_data = true;
        assert!(govern_federated_context_access(&protected).is_err());
        let mut nonlocal = request();
        nonlocal.local_only = false;
        assert!(govern_federated_context_access(&nonlocal).is_err());
    }
}
