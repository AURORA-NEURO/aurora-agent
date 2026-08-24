//! Failure types, split by which party has to move.
//!
//! [`GrantError`] is a refusal of the authority document itself; nothing was planned and nothing
//! ran. [`AutopilotError`] covers everything after a valid grant exists, and its variants keep
//! the 40.36-relevant distinction a CLI needs: a grant that does not authorise the mission it
//! was asked to drive is a policy refusal, while a mission or report that does not parse is
//! invalid input. Collapsing those onto one variant would force the process boundary to guess.

use thiserror::Error;

/// Refusals produced while validating an [`crate::AutonomyGrant`].
///
/// Each variant names the field that must change. There is deliberately no variant for a
/// terminal-retry option: `terminal` outcomes are never re-dispatched, and the absence of a knob
/// is how that rule is made unrepresentable rather than merely defaulted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GrantError {
    #[error("allowed_tools must name at least one tool; there is no default allow-list")]
    NoTools,
    #[error("allowed_tools contains {count} entries; the maximum is {maximum}")]
    TooManyTools { count: usize, maximum: usize },
    #[error("allowed_tools entry {tool:?} is not a bare tool name (ASCII alphanumerics and underscores)")]
    InvalidToolName { tool: String },
    #[error("allowed_tools contains {tool:?} more than once")]
    DuplicateTool { tool: String },
    #[error("a grant cannot allow agent_mission: recursive mission dispatch is refused")]
    RecursiveTool,
    #[error("max_attempts is {value}; the budget must be between 1 and {maximum} total dispatches")]
    InvalidAttemptBudget { value: usize, maximum: usize },
    #[error(
        "retry schedule is invalid: base delay {base}, maximum delay {maximum}; both must be \
         zero for immediate retries or within the {ceiling}-tick bound with maximum >= base"
    )]
    InvalidRetrySchedule {
        base: u64,
        maximum: u64,
        ceiling: u64,
    },
    #[error(
        "stop_on_first_success=false is not supported: re-dispatching a succeeded mission would \
         re-run side effects without a defined evidence meaning, so only true is accepted"
    )]
    UnsupportedStopOption,
}

/// Failures of planning, driving, or report handling under a valid grant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AutopilotError {
    /// The grant refused something the mission requires. The mission is well-formed; the
    /// authority is what is missing, which is the CLI's `policy_denied` conversation.
    #[error("the grant does not authorise this mission: {reason}")]
    GrantDoesNotAuthorise { reason: String },
    /// The mission document does not satisfy the mission contract.
    #[error("invalid mission: {reason}")]
    InvalidMission { reason: String },
    /// A mission report does not satisfy the mission-report contract.
    #[error("invalid mission report: {reason}")]
    InvalidReport { reason: String },
    /// A workflow instantiation artifact does not satisfy the instantiation contract.
    #[error("invalid workflow instantiation: {reason}")]
    InvalidInstantiation { reason: String },
    /// An autopilot report does not satisfy this crate's report contract.
    #[error("invalid autopilot report: {reason}")]
    InvalidAutopilotReport { reason: String },
    /// A restart checkpoint was malformed, tampered with, or did not match the caller's
    /// rehydrated mission/grant/evidence.
    #[error("invalid autopilot checkpoint: {reason}")]
    InvalidCheckpoint { reason: String },
    /// A caller-owned checkpoint store could not be read or written.
    #[error("autopilot checkpoint persistence failed: {reason}")]
    Persistence { reason: String },
    /// A transactional checkpoint store rejected a stale writer.
    #[error("autopilot checkpoint compare-and-swap conflict")]
    CompareAndSwapConflict,
    /// The caller-owned wait boundary could not honor an authorized retry delay.
    #[error("autopilot scheduling wait failed: {reason}")]
    Scheduling { reason: String },
    /// A value could not be canonically encoded for hashing. This is the one failure that is
    /// this crate's fault rather than the caller's, and it is named instead of being swallowed.
    #[error("cannot canonicalise value for digesting: {reason}")]
    Canonicalisation { reason: String },
}
