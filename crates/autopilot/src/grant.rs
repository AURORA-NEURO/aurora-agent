//! The autonomy grant: the one document that authorises autonomous dispatch.
//!
//! The mission executor requires an explicit allow-list per mission; the grant is the same
//! posture lifted to a *drive*: authority is granted once, in a typed document, and every
//! mission the drive dispatches has its policy overwritten from it. There is no default grant,
//! no environment fallback, and no way to widen a grant after construction.
//!
//! The retry options are 40.36's classes, not this crate's invention. Blueprint 40.36 assigns
//! each failure one of three decisions — `terminal`, `retryable_after_change`,
//! `retryable_as_is` — and this document decides which of the *retryable* decisions the drive
//! may act on, plus whether a failure that declared no decision (`unknown`) may be re-sent.
//! There is deliberately no field for retrying `terminal`: a decision 40.36 calls dead-as-written
//! must not be purchasable with a flag, so the illegal state is unrepresentable.
//!
//! [`AutonomyGrant`] has private fields and is constructed only through validation, so holding a
//! grant value *is* the proof it was checked. [`AutonomyGrantDocument`] is the serde-facing JSON
//! contract with `deny_unknown_fields`, because an authority document with a silently ignored
//! field is an authority document the author misread.

use crate::error::GrantError;
use crate::schedule::{RetrySchedule, RetryScheduleDocument};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

/// Largest tool allow-list a grant may carry, matching the mission policy's own ceiling so a
/// grant cannot authorise a list the mission contract would refuse.
pub const MAX_GRANT_TOOLS: usize = 512;
/// Largest total dispatch budget. Sixteen is deliberately small: an autonomous loop that needs
/// more attempts than this is hiding a defect behind persistence.
pub const MAX_GRANT_ATTEMPTS: usize = 16;

fn default_true() -> bool {
    true
}

/// Per-class retry decisions as authored in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicyDocument {
    /// Re-dispatch steps whose recorded evidence declares 40.36 `retryable_as_is`.
    #[serde(default = "default_true")]
    pub retry_retryable_as_is: bool,
    /// Re-dispatch steps whose recorded evidence declares 40.36 `retryable_after_change`. The
    /// only change this drive can make is re-materializing bindings from retained results, which
    /// is why this defaults off: most after-change failures need a budget, grant, or payload the
    /// drive is not authorised to alter.
    #[serde(default)]
    pub retry_retryable_after_change: bool,
    /// Re-dispatch steps whose recorded evidence declares no retry decision at all. Off by
    /// default and deliberately so: an unknown outcome treated as retryable is the classic
    /// dishonest default this workspace exists to refuse.
    #[serde(default)]
    pub retry_unknown: bool,
}

impl Default for RetryPolicyDocument {
    fn default() -> Self {
        RetryPolicyDocument {
            retry_retryable_as_is: true,
            retry_retryable_after_change: false,
            retry_unknown: false,
        }
    }
}

/// The JSON shape of a grant. Unknown fields are refused at parse time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutonomyGrantDocument {
    /// Tools the drive may let missions execute. Required and non-empty: the grant is the only
    /// source of execution authority, so an absent list grants nothing rather than everything.
    pub allowed_tools: Vec<String>,
    /// Permit caller-supplied confirmation flags to reach side-effecting tools.
    #[serde(default)]
    pub allow_side_effects: bool,
    /// Total mission dispatches the drive may perform, full and repair combined.
    pub max_attempts: usize,
    #[serde(default)]
    pub retry: RetryPolicyDocument,
    /// Deterministic caller-clock delay before authorized repair dispatches. Zero preserves
    /// immediate retry behavior; the schedule never grants a retry class the `retry` policy did
    /// not already authorize.
    #[serde(default)]
    pub schedule: RetryScheduleDocument,
    /// Require a reconciliation record with `complete` completion and valid integrity before the
    /// drive may report success. On by default: a mission report alone shows the executor's own
    /// accounting, while reconciliation checks it against the instantiated workflow contract.
    #[serde(default = "default_true")]
    pub require_reconciliation_complete: bool,
    /// Only `true` is accepted in this version. The field exists so the unsupported option is
    /// refused loudly instead of being an unknown field or a silently ignored knob.
    #[serde(default = "default_true")]
    pub stop_on_first_success: bool,
}

/// Validated per-class retry decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    retry_retryable_as_is: bool,
    retry_retryable_after_change: bool,
    retry_unknown: bool,
}

impl RetryPolicy {
    pub fn retryable_as_is(&self) -> bool {
        self.retry_retryable_as_is
    }

    pub fn retryable_after_change(&self) -> bool {
        self.retry_retryable_after_change
    }

    pub fn unknown(&self) -> bool {
        self.retry_unknown
    }
}

/// A validated autonomy grant. Constructed only via [`AutonomyGrant::try_from`] a document (or
/// serde, which routes through the same validation), so an invalid grant value cannot exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AutonomyGrantDocument", into = "AutonomyGrantDocument")]
pub struct AutonomyGrant {
    allowed_tools: Vec<String>,
    allow_side_effects: bool,
    max_attempts: usize,
    retry: RetryPolicy,
    schedule: RetrySchedule,
    require_reconciliation_complete: bool,
}

fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

impl TryFrom<AutonomyGrantDocument> for AutonomyGrant {
    type Error = GrantError;

    fn try_from(document: AutonomyGrantDocument) -> Result<Self, Self::Error> {
        if document.allowed_tools.is_empty() {
            return Err(GrantError::NoTools);
        }
        if document.allowed_tools.len() > MAX_GRANT_TOOLS {
            return Err(GrantError::TooManyTools {
                count: document.allowed_tools.len(),
                maximum: MAX_GRANT_TOOLS,
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for tool in &document.allowed_tools {
            if !valid_tool_name(tool) {
                return Err(GrantError::InvalidToolName { tool: tool.clone() });
            }
            if tool == "agent_mission" {
                return Err(GrantError::RecursiveTool);
            }
            if !seen.insert(tool.clone()) {
                return Err(GrantError::DuplicateTool { tool: tool.clone() });
            }
        }
        if !(1..=MAX_GRANT_ATTEMPTS).contains(&document.max_attempts) {
            return Err(GrantError::InvalidAttemptBudget {
                value: document.max_attempts,
                maximum: MAX_GRANT_ATTEMPTS,
            });
        }
        if !document.stop_on_first_success {
            return Err(GrantError::UnsupportedStopOption);
        }
        let schedule = RetrySchedule::try_from(document.schedule)?;
        Ok(AutonomyGrant {
            allowed_tools: document.allowed_tools,
            allow_side_effects: document.allow_side_effects,
            max_attempts: document.max_attempts,
            retry: RetryPolicy {
                retry_retryable_as_is: document.retry.retry_retryable_as_is,
                retry_retryable_after_change: document.retry.retry_retryable_after_change,
                retry_unknown: document.retry.retry_unknown,
            },
            schedule,
            require_reconciliation_complete: document.require_reconciliation_complete,
        })
    }
}

impl From<AutonomyGrant> for AutonomyGrantDocument {
    fn from(grant: AutonomyGrant) -> Self {
        AutonomyGrantDocument {
            allowed_tools: grant.allowed_tools,
            allow_side_effects: grant.allow_side_effects,
            max_attempts: grant.max_attempts,
            retry: RetryPolicyDocument {
                retry_retryable_as_is: grant.retry.retry_retryable_as_is,
                retry_retryable_after_change: grant.retry.retry_retryable_after_change,
                retry_unknown: grant.retry.retry_unknown,
            },
            schedule: grant.schedule.into(),
            require_reconciliation_complete: grant.require_reconciliation_complete,
            stop_on_first_success: true,
        }
    }
}

impl AutonomyGrant {
    pub fn allowed_tools(&self) -> &[String] {
        &self.allowed_tools
    }

    pub fn allow_side_effects(&self) -> bool {
        self.allow_side_effects
    }

    pub fn max_attempts(&self) -> usize {
        self.max_attempts
    }

    pub fn retry(&self) -> &RetryPolicy {
        &self.retry
    }

    pub fn schedule(&self) -> &RetrySchedule {
        &self.schedule
    }

    pub fn require_reconciliation_complete(&self) -> bool {
        self.require_reconciliation_complete
    }

    /// Canonical content digest of the grant document, chained into every autopilot report so a
    /// reader can check which authority a drive ran under.
    pub fn digest(&self) -> Result<String, crate::error::AutopilotError> {
        let document = AutonomyGrantDocument::from(self.clone());
        let value = serde_json::to_value(document).map_err(|error| {
            crate::error::AutopilotError::Canonicalisation {
                reason: error.to_string(),
            }
        })?;
        ContentHash::of_value(&value)
            .map(|digest| digest.to_string())
            .map_err(|error| crate::error::AutopilotError::Canonicalisation {
                reason: error.to_string(),
            })
    }
}
