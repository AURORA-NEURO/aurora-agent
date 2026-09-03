//! Typed runtime failures.
//!
//! Blueprint 05.03 asks for error classes that distinguish user task failure, environment build
//! failure, provider capacity, policy denial, unsupported capability, corrupted checkpoint and
//! platform bug. The reason it matters is that these are *routed* differently: a policy denial is
//! evidence about the agent, an unsupported capability is evidence about the provider, and only a
//! platform bug is evidence about us. A single stringly-typed error would collapse all three into
//! "run failed", and a benchmark that cannot tell them apart cannot be trusted about either.
//!
//! Every variant here is a *refusal*, not a warning. Nothing in this crate degrades: there is no
//! path where a denied effect returns an empty result, a missing provider falls back to a weaker
//! one, or an exhausted budget truncates the work and reports success.

use crate::budget::RuntimeResource;
use crate::effect::{EffectClass, EffectKind};
use crate::orchestrator::RunState;
use thiserror::Error;

/// The one error type this crate returns.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeError {
    #[error("runtime invariant violated: {detail}")]
    InvariantViolation { detail: String },

    // --- WorldTape and state ledger (05.04) ---
    #[error("tape chain broken at step {step}: {reason}")]
    BrokenChain { step: u64, reason: String },

    #[error("effect payload is not canonically serializable: {0}")]
    Uncanonical(String),

    #[error("step {step} is out of range for a tape of {length} steps")]
    StepOutOfRange { step: u64, length: u64 },

    #[error("checkpoint {id} commits to tape head {expected} but the tape is at {found}")]
    CorruptCheckpoint {
        id: String,
        expected: String,
        found: String,
    },

    // --- Replay and determinism (05.07) ---
    /// The loud failure that the whole virtualization layer exists to produce.
    #[error(
        "replay reached an unrecorded effect at step {step}: {request}; \
         a replay never falls through to the live world"
    )]
    UnrecordedEffect { step: u64, request: String },

    #[error("replay diverged at step {step}: the tape records {recorded}, the program requested {requested}")]
    DivergentRequest {
        step: u64,
        recorded: String,
        requested: String,
    },

    #[error(
        "replay consumed {consumed} of {total} recorded effects; the remainder is unaccounted for"
    )]
    ReplayIncomplete { consumed: u64, total: u64 },

    // --- Effects and permissions (05.08) ---
    #[error("effect {kind} was never declared by the execution plan")]
    UndeclaredEffect { kind: EffectKind },

    #[error("effect {kind} is class {class}, which this policy does not permit")]
    ClassForbidden {
        class: EffectClass,
        kind: EffectKind,
    },

    #[error("path {path} lies outside the declared allowlist")]
    PathDenied { path: String },

    #[error("network mode {mode} refuses host {host}")]
    NetworkDenied { mode: String, host: String },

    #[error("network request names an invalid target URL: {url}")]
    InvalidNetworkUrl { url: String },

    #[error("tape contains too many {kind}: {actual} exceeds the maximum of {maximum}")]
    TapeLimitExceeded {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },

    #[error("tape JSON is {actual} bytes, above the maximum of {maximum}")]
    TapeTooLarge { actual: usize, maximum: usize },

    #[error("tape lineage at step {step} commits to {expected}, but the tape contains {found}")]
    LineageMismatch {
        step: u64,
        expected: String,
        found: String,
    },

    #[error("irreversible effect {kind} refused: an evaluation run may not materialize it")]
    IrreversibleRefused { kind: EffectKind },

    // --- Secret broker (05.08) ---
    #[error("no secret is registered for resource {resource}")]
    UnknownResource { resource: String },

    #[error("capability {id} was not issued by this broker")]
    UnknownCapability { id: String },

    #[error("capability {id} does not match the broker-issued token")]
    CapabilityMismatch { id: String },

    #[error("capability {id} expired at task time {expires_at_task_millis}ms, redeemed at {now_task_millis}ms")]
    CapabilityExpired {
        id: String,
        expires_at_task_millis: u64,
        now_task_millis: u64,
    },

    #[error("capability {id} was revoked")]
    CapabilityRevoked { id: String },

    #[error("capability {id} covers {covered} on {resource}, not {requested}")]
    OperationNotCovered {
        id: String,
        resource: String,
        covered: String,
        requested: String,
    },

    // --- Executor providers (05.03) ---
    #[error(
        "executor provider {provider} is declared but not implemented; \
         {operation} will not silently degrade to another provider"
    )]
    ProviderUnavailable { provider: String, operation: String },

    #[error("provider {provider} does not support capability {capability}")]
    CapabilityUnsupported {
        provider: String,
        capability: String,
    },

    #[error("provider {provider} has no live handle for trial {trial}")]
    UnknownHandle { provider: String, trial: String },

    // --- Fork and suffix execution (05.05) ---
    #[error("cannot fork at step {step}: the parent tape has {length} steps")]
    ForkBeyondEnd { step: u64, length: u64 },

    #[error(
        "the suffix host is still replaying its inherited prefix (step {step} of {inherited})"
    )]
    SuffixNotReached { step: u64, inherited: u64 },

    // --- Budget and cost (05.09) ---
    #[error("resource {resource} has no declared budget; an undeclared resource cannot be spent")]
    UndeclaredResource { resource: RuntimeResource },

    #[error(
        "{resource} budget exhausted: {used} of {hard} used, {requested} more requested; \
         the trial aborts rather than truncating"
    )]
    BudgetExhausted {
        resource: RuntimeResource,
        hard: u64,
        used: u64,
        requested: u64,
    },

    #[error("the trial already aborted on {resource} and cannot resume spending")]
    AlreadyAborted { resource: RuntimeResource },

    #[error(
        "child allocation of {requested} {resource} exceeds the parent's remaining {available}"
    )]
    OverAllocatedChild {
        resource: RuntimeResource,
        requested: u64,
        available: u64,
    },

    // --- Orchestration (05.02) ---
    #[error("illegal lifecycle transition from {from} to {to}")]
    IllegalTransition { from: RunState, to: RunState },

    #[error("run is {state} and cannot accept new work")]
    RunNotRunnable { state: RunState },

    #[error("malformed {kind} identifier: {value:?}")]
    MalformedId { kind: &'static str, value: String },

    // --- The live world itself ---
    /// The world refused or could not answer. Distinct from a policy denial: the trial was allowed
    /// to ask, and the answer did not come.
    #[error("the live world failed to answer {request}: {reason}")]
    SourceFailure { request: String, reason: String },

    /// A fault the fault-injection schedule asked for (05.07). Deterministic by construction, so a
    /// fault-mutation pack reproduces exactly.
    #[error("injected fault at call {call}: {fault}")]
    InjectedFault { call: u64, fault: String },
}
