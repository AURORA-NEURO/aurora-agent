//! Versioned participant exchange for institution-local glioma federation agents.
//!
//! This is a typed protocol seam, not a remote transport implementation. It models capability
//! discovery, proposal review, aggregate contribution, revocation, and receipt retrieval while
//! enforcing API version, policy scope, local approval, revocation, and idempotency. The result is
//! metadata and content digests only; it never exchanges credentials, raw data, or clinical advice.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaFederationParticipantExchange1@1";
pub const API_VERSION: &str = "participant-api/1.0";
pub const MAX_TEXT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantExchangeAction {
    DiscoverCapability,
    ReviewProposal,
    SubmitContribution,
    RevokeParticipation,
    RetrieveReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantExchangeStatus {
    Accepted,
    NeedsApproval,
    Revoked,
    Duplicate,
    Rejected,
    NotFound,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationParticipantRequest {
    pub api_version: String,
    pub exchange_id: String,
    pub idempotency_key: String,
    pub site_id: String,
    pub action: ParticipantExchangeAction,
    pub policy_scope: String,
    pub required_policy_scope: String,
    pub local_approval: bool,
    pub revoked: bool,
    pub capability_manifest_digest: Option<ContentHash>,
    pub proposal_digest: Option<ContentHash>,
    pub contribution_digest: Option<ContentHash>,
    pub receipt_digest: Option<ContentHash>,
    pub replay_of_exchange_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationParticipantReceipt {
    pub receipt_id: String,
    pub exchange_digest: ContentHash,
    pub action: ParticipantExchangeAction,
    pub status: ParticipantExchangeStatus,
    pub site_id: String,
    pub api_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationParticipantExchange {
    pub feature_id: String,
    pub output_schema: String,
    pub api_version: String,
    pub exchange_id: String,
    pub idempotency_key: String,
    pub site_id: String,
    pub action: ParticipantExchangeAction,
    pub status: ParticipantExchangeStatus,
    pub reason_order: Vec<String>,
    pub receipt: Option<FederationParticipantReceipt>,
    pub local_approval_required: bool,
    pub raw_data_moved: bool,
    pub credentials_exchanged: bool,
    pub clinical_decision_made: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParticipantApiError {
    #[error("participant exchange request is invalid: {0}")]
    InvalidRequest(String),
    #[error("participant exchange output is invalid: {0}")]
    InvalidOutput(String),
    #[error("participant exchange digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT && !value.chars().any(char::is_control)
}

fn digest_body(exchange: &FederationParticipantExchange) -> serde_json::Value {
    serde_json::json!({
        "feature_id": exchange.feature_id,
        "output_schema": exchange.output_schema,
        "api_version": exchange.api_version,
        "exchange_id": exchange.exchange_id,
        "idempotency_key": exchange.idempotency_key,
        "site_id": exchange.site_id,
        "action": exchange.action,
        "status": exchange.status,
        "reason_order": exchange.reason_order,
        "receipt": exchange.receipt,
        "local_approval_required": exchange.local_approval_required,
        "raw_data_moved": exchange.raw_data_moved,
        "credentials_exchanged": exchange.credentials_exchanged,
        "clinical_decision_made": exchange.clinical_decision_made,
    })
}

fn request_digest(
    request: &FederationParticipantRequest,
) -> Result<ContentHash, ParticipantApiError> {
    ContentHash::of_value(&serde_json::json!({
        "api_version": request.api_version.clone(),
        "exchange_id": request.exchange_id.clone(),
        "idempotency_key": request.idempotency_key.clone(),
        "site_id": request.site_id.clone(),
        "action": request.action,
        "policy_scope": request.policy_scope.clone(),
        "required_policy_scope": request.required_policy_scope.clone(),
        "local_approval": request.local_approval,
        "revoked": request.revoked,
        "capability_manifest_digest": request.capability_manifest_digest,
        "proposal_digest": request.proposal_digest,
        "contribution_digest": request.contribution_digest,
        "receipt_digest": request.receipt_digest,
        "replay_of_exchange_digest": request.replay_of_exchange_digest,
    }))
    .map_err(|error| ParticipantApiError::Digest(error.to_string()))
}

fn validate_request(request: &FederationParticipantRequest) -> Result<(), ParticipantApiError> {
    if request.api_version != API_VERSION
        || !safe_text(&request.exchange_id)
        || !safe_text(&request.idempotency_key)
        || !safe_text(&request.site_id)
        || !safe_text(&request.policy_scope)
        || !safe_text(&request.required_policy_scope)
    {
        return Err(ParticipantApiError::InvalidRequest(
            "supported API version and bounded exchange, idempotency, site, and policy identity are required".into(),
        ));
    }
    let required_digest = match request.action {
        ParticipantExchangeAction::DiscoverCapability => {
            request.capability_manifest_digest.is_some()
        }
        ParticipantExchangeAction::ReviewProposal => request.proposal_digest.is_some(),
        ParticipantExchangeAction::SubmitContribution => request.contribution_digest.is_some(),
        ParticipantExchangeAction::RevokeParticipation => true,
        ParticipantExchangeAction::RetrieveReceipt => request.receipt_digest.is_some(),
    };
    if !required_digest {
        return Err(ParticipantApiError::InvalidRequest(
            "action-specific content digest is required".into(),
        ));
    }
    Ok(())
}

impl FederationParticipantExchange {
    pub fn validate(&self) -> Result<(), ParticipantApiError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.api_version != API_VERSION
            || !safe_text(&self.exchange_id)
            || !safe_text(&self.idempotency_key)
            || !safe_text(&self.site_id)
            || self.raw_data_moved
            || self.credentials_exchanged
            || self.clinical_decision_made
            || self.digest.as_str().len() != 64
            || self.reason_order.iter().any(|reason| !safe_text(reason))
        {
            return Err(ParticipantApiError::InvalidOutput(
                "participant exchange identity, boundary, reason, or digest invariants are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| ParticipantApiError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ParticipantApiError::InvalidOutput(
                "participant exchange digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Evaluate one versioned participant exchange at an institution-owned protocol boundary.
pub fn execute_glioma_participant_exchange(
    request: &FederationParticipantRequest,
) -> Result<FederationParticipantExchange, ParticipantApiError> {
    validate_request(request)?;
    let mut reasons = Vec::new();
    let status = if request.revoked {
        reasons.push("site participation is revoked".into());
        ParticipantExchangeStatus::Revoked
    } else if request.policy_scope != request.required_policy_scope {
        reasons.push("requested policy scope does not match the site grant".into());
        ParticipantExchangeStatus::Rejected
    } else if request.replay_of_exchange_digest.is_some() {
        reasons.push("idempotent replay maps to the prior exchange identity".into());
        ParticipantExchangeStatus::Duplicate
    } else {
        match request.action {
            ParticipantExchangeAction::DiscoverCapability => ParticipantExchangeStatus::Accepted,
            ParticipantExchangeAction::ReviewProposal => {
                if request.local_approval {
                    ParticipantExchangeStatus::Accepted
                } else {
                    reasons.push("local PI/data-steward review is required".into());
                    ParticipantExchangeStatus::NeedsApproval
                }
            }
            ParticipantExchangeAction::SubmitContribution => {
                if request.local_approval {
                    ParticipantExchangeStatus::Accepted
                } else {
                    reasons.push("local approval is required before contribution".into());
                    ParticipantExchangeStatus::NeedsApproval
                }
            }
            ParticipantExchangeAction::RevokeParticipation => {
                if request.local_approval {
                    ParticipantExchangeStatus::Accepted
                } else {
                    reasons.push("local approval is required to record revocation".into());
                    ParticipantExchangeStatus::NeedsApproval
                }
            }
            ParticipantExchangeAction::RetrieveReceipt => ParticipantExchangeStatus::Accepted,
        }
    };
    let exchange_digest = request_digest(request)?;
    let receipt = if matches!(status, ParticipantExchangeStatus::Accepted) {
        Some(FederationParticipantReceipt {
            receipt_id: format!("receipt-{}", &exchange_digest.as_str()[..16]),
            exchange_digest: exchange_digest.clone(),
            action: request.action,
            status,
            site_id: request.site_id.clone(),
            api_version: API_VERSION.into(),
        })
    } else {
        None
    };
    let mut exchange = FederationParticipantExchange {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        api_version: API_VERSION.into(),
        exchange_id: request.exchange_id.clone(),
        idempotency_key: request.idempotency_key.clone(),
        site_id: request.site_id.clone(),
        action: request.action,
        status,
        reason_order: reasons,
        receipt,
        local_approval_required: matches!(status, ParticipantExchangeStatus::NeedsApproval),
        raw_data_moved: false,
        credentials_exchanged: false,
        clinical_decision_made: false,
        digest: ContentHash::of_bytes(b"unsealed-glioma-participant-exchange"),
    };
    exchange.digest = ContentHash::of_value(&digest_body(&exchange))
        .map_err(|error| ParticipantApiError::Digest(error.to_string()))?;
    exchange.validate()?;
    Ok(exchange)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request(action: ParticipantExchangeAction) -> FederationParticipantRequest {
        FederationParticipantRequest {
            api_version: API_VERSION.into(),
            exchange_id: "exchange-1".into(),
            idempotency_key: "idem-1".into(),
            site_id: "site-a".into(),
            action,
            policy_scope: "glioma-v1".into(),
            required_policy_scope: "glioma-v1".into(),
            local_approval: true,
            revoked: false,
            capability_manifest_digest: Some(digest("capability")),
            proposal_digest: Some(digest("proposal")),
            contribution_digest: Some(digest("contribution")),
            receipt_digest: Some(digest("receipt")),
            replay_of_exchange_digest: None,
        }
    }

    #[test]
    fn capability_discovery_and_contribution_are_accepted_with_receipts() {
        let discovery = execute_glioma_participant_exchange(&request(
            ParticipantExchangeAction::DiscoverCapability,
        ))
        .unwrap();
        assert_eq!(discovery.status, ParticipantExchangeStatus::Accepted);
        assert!(discovery.receipt.is_some());
        let contribution = execute_glioma_participant_exchange(&request(
            ParticipantExchangeAction::SubmitContribution,
        ))
        .unwrap();
        assert_eq!(contribution.status, ParticipantExchangeStatus::Accepted);
    }

    #[test]
    fn proposal_and_contribution_cannot_bypass_local_approval() {
        let mut proposal = request(ParticipantExchangeAction::ReviewProposal);
        proposal.local_approval = false;
        let review = execute_glioma_participant_exchange(&proposal).unwrap();
        assert_eq!(review.status, ParticipantExchangeStatus::NeedsApproval);
        assert!(review.receipt.is_none());
        assert!(review.local_approval_required);
        let mut contribution = request(ParticipantExchangeAction::SubmitContribution);
        contribution.local_approval = false;
        assert_eq!(
            execute_glioma_participant_exchange(&contribution)
                .unwrap()
                .status,
            ParticipantExchangeStatus::NeedsApproval
        );
    }

    #[test]
    fn revoked_and_mismatched_policy_requests_are_rejected() {
        let mut revoked = request(ParticipantExchangeAction::RetrieveReceipt);
        revoked.revoked = true;
        assert_eq!(
            execute_glioma_participant_exchange(&revoked)
                .unwrap()
                .status,
            ParticipantExchangeStatus::Revoked
        );
        let mut mismatch = request(ParticipantExchangeAction::DiscoverCapability);
        mismatch.policy_scope = "other-v1".into();
        assert_eq!(
            execute_glioma_participant_exchange(&mismatch)
                .unwrap()
                .status,
            ParticipantExchangeStatus::Rejected
        );
    }

    #[test]
    fn idempotent_replays_are_duplicates_not_new_operations() {
        let mut replay = request(ParticipantExchangeAction::SubmitContribution);
        replay.replay_of_exchange_digest = Some(digest("prior-exchange"));
        let output = execute_glioma_participant_exchange(&replay).unwrap();
        assert_eq!(output.status, ParticipantExchangeStatus::Duplicate);
        assert!(output.receipt.is_none());
    }

    #[test]
    fn exchange_replay_is_content_deterministic() {
        let req = request(ParticipantExchangeAction::DiscoverCapability);
        let first = execute_glioma_participant_exchange(&req).unwrap();
        let second = execute_glioma_participant_exchange(&req).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.digest, second.digest);
    }
}
