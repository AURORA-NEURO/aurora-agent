//! Autonomous mission driving with receipts: a typed grant, a pure planner, and a drive loop
//! that never claims more than its retained evidence.
//!
//! The workspace already classifies failures for retry — blueprint 40.36's three-valued retry
//! decision is transcribed in `bioprism-devx`, republished by the CLI exit-code registry, and
//! carried in every `--json` error envelope — but nothing *consumed* those decisions. Every
//! recovery surface stopped at "a human re-reads the report and re-sends". This crate is the
//! consumer: given an instantiated mission and an explicit [`AutonomyGrant`], it runs
//! plan → dispatch → classify → repair cycles until the workflow's evidence is complete, the
//! attempt budget is spent, or something is refused in a way that makes re-sending dishonest.
//!
//! 40.36 specifies the retry classification. It does not specify an autonomous driver, an
//! authority document, or a repair-subset construction; those are this crate's design, stated as
//! such rather than attributed to the blueprint. The CLI contract this kernel will sit behind is
//! blueprint 40.13's concern and is deliberately not implemented here.
//!
//! # Authority comes from the grant, and only from the grant
//!
//! There is no default grant. [`AutonomyGrant`] is a validated document naming the tool
//! allow-list, the side-effect posture, the total dispatch budget, and which 40.36 retry classes
//! may actually be re-dispatched. Its constructor refuses an empty allow-list, a zero or
//! over-sixteen attempt budget, and unknown fields. The grant's authority is applied by
//! *overwriting* the dispatched mission's policy — a mission authored with a wider allow-list
//! than the grant is narrowed, never widened.
//!
//! # Honest labelling rules this crate enforces
//!
//! - **A refusal is terminal.** A step the executor refused was refused by policy behaving
//!   correctly; re-sending the identical step is dishonest and no grant option enables it.
//! - **Unknown is never retryable by default.** A failed step whose recorded evidence declares no
//!   40.36 retry decision classifies as `unknown`, and `unknown` is re-dispatched only under an
//!   explicit `retry.retry_unknown = true`.
//! - **Success requires evidence, never inference.** [`plan_next_action`] returns
//!   [`NextAction::StopSuccess`] only when every planned step has a retained `succeeded` result
//!   and — under the default `require_reconciliation_complete` — the final attempt carries a
//!   reconciliation record whose completion is `complete` with valid integrity.
//! - **A cancellation is an authority, not a failure mode.** A cancelled mission stops the drive
//!   regardless of the grant's retry options, and a cancelled step is never re-dispatched even
//!   inside a report whose mission status is not `cancelled`; the autopilot never overrides an
//!   operator.
//! - **Exhaustion is unrepresentable as a dispatch.** [`NextAction`]'s dispatch variants carry a
//!   [`DispatchAuthorization`] token with no public constructor; the planner mints one only while
//!   dispatches used are below the grant's budget.
//!
//! # Shape of the crate
//!
//! [`grant`], [`classify`], [`history`], [`planner`], [`report`], and the checkpoint projection are
//! the pure kernel: no I/O, no clock, no randomness, every digest a function of its input.
//! [`drive`] is the effectful module, and its only execution effect is calling a caller-supplied
//! [`drive::MissionDispatch`] — in production a closure over the in-process MCP server's
//! `execute_agent_mission` boundary, in tests a fake. The checkpoint store is caller-owned: this
//! crate supplies strict JSON validation and compare-and-swap orchestration, never a file,
//! database, or credential implementation.
//!
//! # Not implemented
//!
//! - **No recurrence.** A drive runs one mission to a stop state in one call. Bounded retry
//!   backoff is available through [`RetrySchedule`] and a caller-owned [`AutopilotWait`] seam,
//!   but this crate does not repeat a completed mission.
//! - **No MCP tool exposure.** This crate is not an MCP tool and registers nothing with the
//!   server; the drive *calls* the mission boundary through a seam the caller supplies.
//! - **Metadata-only cross-process resume.** [`persistence`] seals a bounded checkpoint after
//!   each dispatch and [`drive::resume_mission_with_checkpoint`] verifies caller-rehydrated
//!   private attempts before planning continues. The checkpoint intentionally does not retain
//!   mission arguments, provider output, credentials, or evidence; a caller that cannot rehydrate
//!   those values must stop rather than guess.
//! - **No wall-clock ownership or deadlines.** 40.36's `retryable_as_is` means "may succeed
//!   later"; the grant can authorize deterministic logical-tick backoff, while the caller decides
//!   how to wait and whether a deadline has elapsed.
//! - **No retry of an undelivered dispatch.** A transport error leaves the mission outcome
//!   unknown at mission level — side effects may have run — so the drive stops rather than
//!   re-sending blind.
//! - **No re-dispatch of a succeeded step.** A repair re-materializes bindings from retained
//!   results; when a needed payload was not retained, the dependent step is excluded and the
//!   reason recorded, never "fixed" by re-running its already-succeeded dependency.
//! - **No whole-plan reconciliation after a repair.** A repair attempt's reconciliation covers
//!   exactly the re-dispatched subset and is labelled with that scope; the crate never
//!   fabricates a merged mission report to make the original instantiation reconcile.
//! - **No claim lineage past attempt 1.** A repair re-dispatches steps without the base
//!   mission's claim requests or reviews; the stripped claim ids are disclosed on the repair
//!   dispatch action, and the limitation is stated in every report.

pub mod classify;
pub mod drive;
pub mod error;
pub mod grant;
pub mod history;
pub mod planner;
pub mod persistence;
pub mod report;
pub mod schedule;

pub use classify::{classify_step_result, RetryClass, StepClass, StepClassification};
pub use drive::{
    drive_instantiation, drive_instantiation_with_checkpoint, drive_instantiation_with_schedule,
    drive_mission, drive_mission_with_checkpoint, drive_mission_with_schedule,
    resume_instantiation_with_checkpoint, resume_instantiation_with_schedule,
    resume_mission_with_checkpoint, resume_mission_with_schedule, DriveOutcome, MissionDispatch,
};
pub use error::{AutopilotError, GrantError};
pub use grant::{AutonomyGrant, AutonomyGrantDocument, RetryPolicy, RetryPolicyDocument};
pub use history::{AttemptKind, AttemptRecord, DriveHistory};
pub use planner::{plan_next_action, preview_first_action, DispatchAuthorization, NextAction};
pub use persistence::{
    attempt_checkpoint_projection, restore_drive_history, seal_autopilot_checkpoint,
    validate_autopilot_checkpoint, AutopilotCheckpointPersistence,
    AutopilotCheckpointPersistenceCoordinator, AutopilotCheckpointStore,
    JsonAutopilotCheckpointPersistence, TransactionalAutopilotCheckpointPersistence,
    TransactionalAutopilotCheckpointPersistenceCoordinator,
    TransactionalAutopilotCheckpointStore, TransactionalJsonAutopilotCheckpointPersistence,
    AUTOPILOT_CHECKPOINT_MAX_ATTEMPTS, AUTOPILOT_CHECKPOINT_MAX_BYTES,
    AUTOPILOT_CHECKPOINT_MAX_STEP_IDS, AUTOPILOT_CHECKPOINT_RETENTION,
    AUTOPILOT_CHECKPOINT_SCHEMA,
};
pub use report::{
    build_autopilot_report, verify_autopilot_report, FinalDisposition, FinalStatus,
    AUTOPILOT_REPORT_SCHEMA_VERSION, REQUIRED_LIMITATIONS,
};
pub use schedule::{AutopilotWait, RetrySchedule, RetryScheduleDocument, MAX_RETRY_DELAY_TICKS};
