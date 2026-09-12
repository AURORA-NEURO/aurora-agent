use thiserror::Error;

/// Fail-closed errors from campaign validation, transitions, and receipt adapters.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CampaignError {
    #[error("invalid campaign specification: {reason}")]
    InvalidSpec { reason: String },
    #[error("invalid campaign receipt: {reason}")]
    InvalidReceipt { reason: String },
    #[error("invalid campaign reconciliation receipt: {reason}")]
    InvalidReconciliationReceipt { reason: String },
    #[error("invalid campaign checkpoint: {reason}")]
    InvalidCheckpoint { reason: String },
    #[error("campaign canonicalisation failed: {reason}")]
    Canonicalisation { reason: String },
    #[error("{kind} requires the disabled Cargo feature {required_feature}")]
    AdapterUnavailable {
        kind: String,
        required_feature: &'static str,
    },
    #[error("campaign status {status} cannot authorize another action")]
    ActionNotAvailable { status: String },
    #[error("the campaign authorization checkpoint transaction was rejected: {reason}")]
    AuthorizationCheckpointRejected { reason: String },
    #[error("another campaign action is already in flight")]
    ActionAlreadyInFlight,
    #[error("campaign action authorization is stale or belongs to another campaign")]
    StaleAuthorization,
    #[error("campaign reconciliation receipt is stale or belongs to another action")]
    StaleReconciliationReceipt,
    #[error("campaign status {status} has no action available for reconciliation")]
    ReconciliationNotAvailable { status: String },
    #[error("the configured campaign execution journal rejected reconciliation: {reason}")]
    ReconciliationJournalRejected { reason: String },
    #[error("campaign action ceiling is exhausted")]
    ActionCeilingExhausted,
    #[error("{component} refused or failed to verify its artifact: {reason}")]
    Upstream {
        component: &'static str,
        reason: String,
    },
}

pub(crate) fn invalid_spec(reason: impl Into<String>) -> CampaignError {
    CampaignError::InvalidSpec {
        reason: reason.into(),
    }
}

pub(crate) fn invalid_receipt(reason: impl Into<String>) -> CampaignError {
    CampaignError::InvalidReceipt {
        reason: reason.into(),
    }
}

pub(crate) fn invalid_reconciliation_receipt(reason: impl Into<String>) -> CampaignError {
    CampaignError::InvalidReconciliationReceipt {
        reason: reason.into(),
    }
}

pub(crate) fn invalid_checkpoint(reason: impl Into<String>) -> CampaignError {
    CampaignError::InvalidCheckpoint {
        reason: reason.into(),
    }
}
