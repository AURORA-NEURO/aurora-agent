//! Transactions, sagas, compensating actions and partial failure.
//!
//! Blueprint 23.21 and 23.22. Agent teams touch systems that cannot share one database
//! transaction, so the coordination pattern is a saga: run steps forward, and on failure run the
//! compensations backward.
//!
//! # Compensation does not mean rollback
//!
//! The honest version of the pattern, and the one that matters for a system performing real
//! research actions, is 23.21's own sentence: *"Compensation may not perfectly restore the prior
//! world."* Yanking a package version does not unpublish it from anyone's cache. Deleting a branch
//! does not retract the notification. So [`Compensation`] declares a [`Restoration`] level, a
//! `Partial` level must list its [`Residual`]s, and the outcome type keeps the two cases apart:
//! [`SagaOutcome::CompensatedFully`] and [`SagaOutcome::CompensatedWithResidue`] are different
//! variants, and [`SagaOutcome::restored_prior_state`] is true only for the first. A saga that
//! reported "rolled back" when three of five effects were still in the world would be lying in
//! exactly the direction that gets somebody hurt.
//!
//! # Unobserved is not failed and is not committed
//!
//! [`StepOutcome::Unobserved`] is 23.21's "agent claims success before observing commit". The
//! effect may or may not have happened. Compensation still runs — it must — but the step's report
//! carries `observed: false` and compensating it adds a residual saying the compensation targeted
//! an effect nobody confirmed. Folding that into either success or failure would destroy the only
//! information a human needs to go and look.
//!
//! # Unreachable compensation
//!
//! [`Saga::run`] takes the authority actually available *in the failure state*, separately from
//! the authority the saga declared it would hold. That is how 23.21's "revocation between prepare
//! and commit" and 23.45's "unreachable compensation" become a reachable outcome
//! ([`SagaOutcome::CompensationBlocked`]) rather than a panic: a saga that validated cleanly can
//! still find, at compensation time, that the grant it needed is gone.
//!
//! # Deliberately not implemented
//!
//! Nothing here performs an effect. There is no executor, no retry loop, no transport, no clock
//! and no randomness; [`Saga::run`] consumes a caller-supplied outcome script, which is precisely
//! what makes the chaos scenarios of 23.22 (dropout, duplicate delivery, stale version,
//! revocation) reproducible tests instead of flaky ones. Local begin/commit/rollback against a
//! real transactional resource, dry-run and shadow-resource execution modes, lease and heartbeat
//! timing, and Byzantine containment are all out of scope. 23.22's recovery ladder mentions
//! "resume from latest valid continuation" and "fork from an earlier checkpoint"; continuations
//! belong to the weave kernel (23.10), so [`Recovery::Resume`] names the rung and stops rather
//! than reimplementing it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 23.21's effect taxonomy. Every step declares one, because the safe recovery for a step depends
/// entirely on which of these it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    ReadOnly,
    IdempotentWrite,
    ReversibleWrite,
    CompensatableExternal,
    Irreversible,
}

impl ActionClass {
    /// Whether repeating the action is guaranteed harmless.
    ///
    /// A reversible write is not idempotent: reversible means it can be undone afterwards, which
    /// says nothing about running it twice.
    pub fn is_idempotent(self) -> bool {
        matches!(self, ActionClass::ReadOnly | ActionClass::IdempotentWrite)
    }

    /// Whether the class implies an effect that outlives the step.
    pub fn has_effect(self) -> bool {
        !matches!(self, ActionClass::ReadOnly)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ActionClass::ReadOnly => "read_only",
            ActionClass::IdempotentWrite => "idempotent_write",
            ActionClass::ReversibleWrite => "reversible_write",
            ActionClass::CompensatableExternal => "compensatable_external",
            ActionClass::Irreversible => "irreversible",
        }
    }
}

/// How much of the prior world a compensation puts back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Restoration {
    /// The prior state is restored exactly. Claiming this is a strong claim and the validator
    /// checks that no residue is listed alongside it.
    Complete,
    /// Some of the effect remains. The residue must be named.
    Partial,
}

/// What follow-up a residual effect needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "remediation", rename_all = "snake_case")]
pub enum Remediation {
    /// The residue is known, bounded and acceptable.
    NotRequired,
    /// Somebody has to do something about it.
    FollowUpRequired { action: String },
    /// Somebody has to be told about it.
    HumanNotification { audience: String },
}

/// One thing a compensation could not undo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Residual {
    pub description: String,
    pub remediation: Remediation,
}

impl Residual {
    pub fn new(description: impl Into<String>, remediation: Remediation) -> Self {
        Residual {
            description: description.into(),
            remediation,
        }
    }
}

/// The undo action for a step, with its honesty declared up front.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compensation {
    pub name: String,
    pub restoration: Restoration,
    /// What remains after compensation. Required to be non-empty for [`Restoration::Partial`] and
    /// required to be empty for [`Restoration::Complete`].
    #[serde(default)]
    pub residuals: Vec<Residual>,
    /// A grant the compensation needs. If it is not held in the failure state, the compensation
    /// is unreachable.
    #[serde(default)]
    pub requires_authority: Option<String>,
}

impl Compensation {
    pub fn complete(name: impl Into<String>) -> Self {
        Compensation {
            name: name.into(),
            restoration: Restoration::Complete,
            residuals: Vec::new(),
            requires_authority: None,
        }
    }

    pub fn partial(name: impl Into<String>, residuals: Vec<Residual>) -> Self {
        Compensation {
            name: name.into(),
            restoration: Restoration::Partial,
            residuals,
            requires_authority: None,
        }
    }

    pub fn needing_authority(mut self, grant: impl Into<String>) -> Self {
        self.requires_authority = Some(grant.into());
        self
    }
}

/// The declared failure policy of 23.21. Constrains what the recovery ladder is allowed to pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum FailurePolicy {
    /// Transient and idempotent. Only legal on a step whose class is idempotent.
    Retry,
    /// A continuation exists to resume from.
    Resume,
    /// Undo the completed steps.
    Compensate,
    /// The artifact or participant is untrusted; stop and hold it.
    Quarantine,
    /// Policy or authority is required.
    Escalate { to: String },
    /// A declared partial result is useful on its own.
    AcceptPartial,
    /// A safety or invariant violation. Undo and stop.
    Abort,
}

/// One step of a saga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub class: ActionClass,
    /// Stable across retries, so at-least-once delivery cannot turn one intent into two effects.
    pub idempotency_key: String,
    #[serde(default)]
    pub compensation: Option<Compensation>,
    pub failure_policy: FailurePolicy,
    /// 23.21 separates prepare from commit for high-impact work. An irreversible step must carry
    /// this or the saga does not validate.
    #[serde(default)]
    pub requires_approval: bool,
}

impl Step {
    pub fn new(
        name: impl Into<String>,
        class: ActionClass,
        idempotency_key: impl Into<String>,
        failure_policy: FailurePolicy,
    ) -> Self {
        Step {
            name: name.into(),
            class,
            idempotency_key: idempotency_key.into(),
            compensation: None,
            failure_policy,
            requires_approval: false,
        }
    }

    pub fn compensated_by(mut self, compensation: Compensation) -> Self {
        self.compensation = Some(compensation);
        self
    }

    pub fn approved(mut self) -> Self {
        self.requires_approval = true;
        self
    }
}

/// What went wrong, in the vocabulary of 23.22's failure taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    Timeout,
    Crash,
    MalformedResponse,
    ToolUnavailable,
    /// The resource moved under the step's feet.
    StaleVersion,
    PolicyDenied,
    /// A safety or correctness invariant broke.
    InvariantViolated,
    /// The step's effect was never confirmed either way.
    Unobserved,
}

/// A rung of 23.22's recovery ladder, with the reason it was chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "recovery", rename_all = "snake_case")]
pub enum Recovery {
    Retry { because: String },
    Resume { because: String },
    Rebind { because: String },
    Quarantine { because: String },
    Compensate { because: String },
    /// Continue with an explicitly downgraded guarantee (23.22, graceful degradation). The
    /// downgrade is named, because a silent one is indistinguishable from a bug.
    Degrade { guarantee: String, because: String },
    Escalate { to: String, because: String },
    /// Stop and return a typed partial result.
    TerminatePartial { because: String },
}

impl Recovery {
    /// Whether this rung ends the saga by undoing what was done.
    pub fn unwinds(&self) -> bool {
        matches!(
            self,
            Recovery::Compensate { .. } | Recovery::TerminatePartial { .. }
        )
    }
}

/// Chooses a recovery rung. Pure, total and deterministic.
///
/// The declared [`FailurePolicy`] is authoritative wherever it names a terminal choice; the ladder
/// only has discretion between retrying, resuming and rebinding, and it never has the discretion
/// to retry a step whose class is not idempotent. That is 23.21's "unsafe retry of non-idempotent
/// action", and it is the one rule here that cannot be relaxed by policy: a policy asking for a
/// retry of a non-idempotent step is a policy that will double-charge somebody.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryLadder {
    pub escalate_to: String,
}

impl Default for RecoveryLadder {
    fn default() -> Self {
        RecoveryLadder {
            escalate_to: "human-reviewer".into(),
        }
    }
}

impl RecoveryLadder {
    pub fn new(escalate_to: impl Into<String>) -> Self {
        RecoveryLadder {
            escalate_to: escalate_to.into(),
        }
    }

    pub fn select(&self, step: &Step, failure: Failure) -> Recovery {
        if failure == Failure::InvariantViolated {
            return Recovery::Compensate {
                because: "an invariant broke, so no forward path is safe regardless of the \
                          declared policy"
                    .into(),
            };
        }
        if failure == Failure::PolicyDenied {
            return Recovery::Escalate {
                to: self.escalate_to.clone(),
                because: "the step was denied by policy, and only an authority can change that"
                    .into(),
            };
        }

        match &step.failure_policy {
            FailurePolicy::Escalate { to } => Recovery::Escalate {
                to: to.clone(),
                because: "the step declares escalation as its failure policy".into(),
            },
            FailurePolicy::Quarantine => Recovery::Quarantine {
                because: "the step declares quarantine, so its artifact is held rather than used"
                    .into(),
            },
            FailurePolicy::AcceptPartial => Recovery::TerminatePartial {
                because: "the step declares that a partial result is useful on its own".into(),
            },
            FailurePolicy::Compensate | FailurePolicy::Abort => Recovery::Compensate {
                because: "the step declares that failure unwinds the saga".into(),
            },
            FailurePolicy::Resume => Recovery::Resume {
                because: "the step declares that a continuation exists to resume from".into(),
            },
            FailurePolicy::Retry => {
                if !step.class.is_idempotent() {
                    return Recovery::Compensate {
                        because: format!(
                            "the step declares retry but its class is {}, which is not \
                             idempotent; repeating it could duplicate the effect",
                            step.class.as_str()
                        ),
                    };
                }
                match failure {
                    Failure::MalformedResponse => Recovery::Rebind {
                        because: "the participant is producing output the schema rejects, so \
                                  repeating the request will not help"
                            .into(),
                    },
                    Failure::Crash => Recovery::Resume {
                        because: "the participant crashed, so the work resumes rather than \
                                  restarts"
                            .into(),
                    },
                    _ => Recovery::Retry {
                        because: "the failure is transient and the step is idempotent under its \
                                  stable key"
                            .into(),
                    },
                }
            }
        }
    }
}

/// What a saga is structurally not allowed to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum SagaError {
    #[error("step {step} is {class} but declares no compensation, so a later failure could not undo it")]
    MissingCompensation { step: String, class: String },

    #[error("step {step} declares partial restoration without naming what it cannot undo")]
    UnnamedResidue { step: String },

    #[error("step {step} claims complete restoration while listing residue; one of the two is wrong")]
    ResidueOnCompleteRestoration { step: String },

    #[error("step {step} is irreversible and does not require approval before execution")]
    UnapprovedIrreversibleStep { step: String },

    #[error("step {step} is irreversible yet declares compensation {compensation}; an irreversible effect has no undo by definition")]
    CompensationOnIrreversibleStep {
        step: String,
        compensation: String,
    },

    #[error("compensation for step {step} needs authority {authority}, which the saga does not declare holding")]
    UnreachableCompensation { step: String, authority: String },

    #[error("steps {step} and {other} share idempotency key {key}; at-least-once delivery could collapse them into one effect")]
    DuplicateIdempotencyKey {
        step: String,
        other: String,
        key: String,
    },

    #[error("step {step} has an effect but no idempotency key, so a redelivery cannot be recognised")]
    MissingIdempotencyKey { step: String },

    #[error("step {step} declares retry but its class {class} is not idempotent")]
    RetryOnNonIdempotentStep { step: String, class: String },

    #[error("the outcome script ends at step {step}, so the saga has no recorded result for it")]
    OutcomeScriptTooShort { step: String },
}

/// A sequence of steps with compensations, plus the authority the saga was granted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saga {
    pub name: String,
    pub steps: Vec<Step>,
    /// Grants the saga declares it holds. Checked against each compensation's requirement at
    /// validation time; what is actually available at compensation time is a separate argument to
    /// [`Saga::run`], because grants are revocable (23.13).
    #[serde(default)]
    pub declared_authority: BTreeSet<String>,
}

impl Saga {
    pub fn new(name: impl Into<String>, steps: Vec<Step>) -> Self {
        Saga {
            name: name.into(),
            steps,
            declared_authority: BTreeSet::new(),
        }
    }

    pub fn holding(mut self, grant: impl Into<String>) -> Self {
        self.declared_authority.insert(grant.into());
        self
    }

    /// Every structural problem, in step order. Returning all of them rather than the first is
    /// the difference between one review round and five.
    pub fn problems(&self) -> Vec<SagaError> {
        let mut problems = Vec::new();
        for (index, step) in self.steps.iter().enumerate() {
            if step.class.has_effect() && step.idempotency_key.trim().is_empty() {
                problems.push(SagaError::MissingIdempotencyKey {
                    step: step.name.clone(),
                });
            }
            for other in &self.steps[..index] {
                if other.idempotency_key == step.idempotency_key
                    && !step.idempotency_key.trim().is_empty()
                {
                    problems.push(SagaError::DuplicateIdempotencyKey {
                        step: step.name.clone(),
                        other: other.name.clone(),
                        key: step.idempotency_key.clone(),
                    });
                }
            }

            if step.failure_policy == FailurePolicy::Retry && !step.class.is_idempotent() {
                problems.push(SagaError::RetryOnNonIdempotentStep {
                    step: step.name.clone(),
                    class: step.class.as_str().to_string(),
                });
            }

            match (step.class, &step.compensation) {
                (ActionClass::Irreversible, Some(compensation)) => {
                    problems.push(SagaError::CompensationOnIrreversibleStep {
                        step: step.name.clone(),
                        compensation: compensation.name.clone(),
                    });
                }
                (ActionClass::Irreversible, None) => {
                    if !step.requires_approval {
                        problems.push(SagaError::UnapprovedIrreversibleStep {
                            step: step.name.clone(),
                        });
                    }
                }
                (ActionClass::ReversibleWrite | ActionClass::CompensatableExternal, None) => {
                    problems.push(SagaError::MissingCompensation {
                        step: step.name.clone(),
                        class: step.class.as_str().to_string(),
                    });
                }
                _ => {}
            }

            if let Some(compensation) = &step.compensation {
                match compensation.restoration {
                    Restoration::Partial if compensation.residuals.is_empty() => {
                        problems.push(SagaError::UnnamedResidue {
                            step: step.name.clone(),
                        });
                    }
                    Restoration::Complete if !compensation.residuals.is_empty() => {
                        problems.push(SagaError::ResidueOnCompleteRestoration {
                            step: step.name.clone(),
                        });
                    }
                    _ => {}
                }
                if let Some(authority) = &compensation.requires_authority {
                    if !self.declared_authority.contains(authority) {
                        problems.push(SagaError::UnreachableCompensation {
                            step: step.name.clone(),
                            authority: authority.clone(),
                        });
                    }
                }
            }
        }
        problems
    }

    /// The first problem, or nothing.
    pub fn validate(&self) -> Result<(), SagaError> {
        match self.problems().into_iter().next() {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }

    /// Runs the saga against a caller-supplied outcome script.
    ///
    /// `available_authority` is what is held *now*, in the failure state, which is not necessarily
    /// what [`Saga::declared_authority`] said at planning time. Passing the two separately is how a
    /// revocation between prepare and commit becomes visible instead of being assumed away.
    pub fn run(
        &self,
        script: &[StepOutcome],
        available_authority: &BTreeSet<String>,
        ladder: &RecoveryLadder,
    ) -> Result<SagaOutcome, SagaError> {
        self.validate()?;

        let mut reports: Vec<StepReport> = Vec::new();
        for (index, step) in self.steps.iter().enumerate() {
            let outcome = script
                .get(index)
                .ok_or_else(|| SagaError::OutcomeScriptTooShort {
                    step: step.name.clone(),
                })?;

            match outcome {
                StepOutcome::Succeeded => {
                    reports.push(StepReport {
                        step: step.name.clone(),
                        observed: true,
                        succeeded: true,
                        detail: String::new(),
                        compensation: None,
                    });
                }
                StepOutcome::Unobserved { detail } => {
                    reports.push(StepReport {
                        step: step.name.clone(),
                        observed: false,
                        succeeded: false,
                        detail: detail.clone(),
                        compensation: None,
                    });
                    let recovery = ladder.select(step, Failure::Unobserved);
                    return Ok(self.finish(
                        step,
                        Failure::Unobserved,
                        recovery,
                        reports,
                        available_authority,
                    ));
                }
                StepOutcome::Failed { failure, detail } => {
                    reports.push(StepReport {
                        step: step.name.clone(),
                        observed: true,
                        succeeded: false,
                        detail: detail.clone(),
                        compensation: None,
                    });
                    let recovery = ladder.select(step, *failure);
                    return Ok(self.finish(
                        step,
                        *failure,
                        recovery,
                        reports,
                        available_authority,
                    ));
                }
            }
        }

        Ok(SagaOutcome::Committed { steps: reports })
    }

    /// Convenience for the case where nothing was revoked.
    pub fn run_with_declared_authority(
        &self,
        script: &[StepOutcome],
        ladder: &RecoveryLadder,
    ) -> Result<SagaOutcome, SagaError> {
        self.run(script, &self.declared_authority, ladder)
    }

    fn finish(
        &self,
        failed_step: &Step,
        failure: Failure,
        recovery: Recovery,
        mut reports: Vec<StepReport>,
        available_authority: &BTreeSet<String>,
    ) -> SagaOutcome {
        if !recovery.unwinds() {
            return SagaOutcome::Suspended {
                at: failed_step.name.clone(),
                failure,
                recovery,
                steps: reports,
            };
        }

        let partial = matches!(recovery, Recovery::TerminatePartial { .. });
        let mut residue: Vec<Residual> = Vec::new();
        let mut blocked: Option<(String, String)> = None;

        for report in reports.iter_mut().rev() {
            let step = match self.steps.iter().find(|step| step.name == report.step) {
                Some(step) => step,
                None => continue,
            };
            if !step.class.has_effect() {
                continue;
            }
            if !report.succeeded && report.observed {
                continue;
            }
            let compensation = match &step.compensation {
                Some(compensation) => compensation,
                None => continue,
            };
            if let Some(authority) = &compensation.requires_authority {
                if !available_authority.contains(authority) {
                    report.compensation = Some(CompensationReport {
                        name: compensation.name.clone(),
                        executed: false,
                        restoration: None,
                        residuals: Vec::new(),
                        blocked: Some(format!("authority {authority} is not held")),
                    });
                    if blocked.is_none() {
                        blocked = Some((step.name.clone(), format!("authority {authority} is not held")));
                    }
                    continue;
                }
            }

            let mut residuals = compensation.residuals.clone();
            if !report.observed {
                residuals.push(Residual::new(
                    format!(
                        "compensation {} ran against an effect that was never confirmed to have \
                         happened",
                        compensation.name
                    ),
                    Remediation::HumanNotification {
                        audience: "saga owner".into(),
                    },
                ));
            }
            residue.extend(residuals.iter().cloned());
            report.compensation = Some(CompensationReport {
                name: compensation.name.clone(),
                executed: true,
                restoration: Some(compensation.restoration),
                residuals,
                blocked: None,
            });
        }

        if let Some((step, reason)) = blocked {
            return SagaOutcome::CompensationBlocked {
                failed_at: failed_step.name.clone(),
                failure,
                blocked_at: step,
                reason,
                steps: reports,
                residue,
            };
        }
        if partial {
            return SagaOutcome::PartiallyAccepted {
                stopped_at: failed_step.name.clone(),
                failure,
                steps: reports,
                residue,
            };
        }
        if residue.is_empty() {
            return SagaOutcome::CompensatedFully {
                failed_at: failed_step.name.clone(),
                failure,
                steps: reports,
            };
        }
        SagaOutcome::CompensatedWithResidue {
            failed_at: failed_step.name.clone(),
            failure,
            steps: reports,
            residue,
        }
    }
}

/// What the caller says happened to a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum StepOutcome {
    Succeeded,
    /// The step definitively did not take effect, so it is not compensated: undoing something
    /// that never happened is its own kind of damage. A caller that cannot rule out a partial
    /// effect must report [`StepOutcome::Unobserved`] instead.
    Failed { failure: Failure, detail: String },
    /// The effect may or may not have happened and nobody can tell. Compensation runs anyway.
    Unobserved { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationReport {
    pub name: String,
    pub executed: bool,
    /// Absent when the compensation did not run.
    pub restoration: Option<Restoration>,
    pub residuals: Vec<Residual>,
    /// Why the compensation could not run, if it could not.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepReport {
    pub step: String,
    /// False when the step's result was never confirmed.
    pub observed: bool,
    pub succeeded: bool,
    pub detail: String,
    pub compensation: Option<CompensationReport>,
}

/// How a saga ended.
///
/// The two compensated variants are separate on purpose. Merging them behind a boolean would let
/// a caller write `if rolled_back { forget_about_it() }` over a world that still contains three of
/// the five effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "saga_outcome", rename_all = "snake_case")]
pub enum SagaOutcome {
    Committed {
        steps: Vec<StepReport>,
    },
    /// Every compensation ran and every one of them restores completely.
    CompensatedFully {
        failed_at: String,
        failure: Failure,
        steps: Vec<StepReport>,
    },
    /// Compensation ran and the world is not what it was. The residue says how.
    CompensatedWithResidue {
        failed_at: String,
        failure: Failure,
        steps: Vec<StepReport>,
        residue: Vec<Residual>,
    },
    /// A compensation could not run at all, so part of the effect stands with no undo available.
    CompensationBlocked {
        failed_at: String,
        failure: Failure,
        blocked_at: String,
        reason: String,
        steps: Vec<StepReport>,
        residue: Vec<Residual>,
    },
    /// The declared policy said the partial result is useful. It is still partial.
    PartiallyAccepted {
        stopped_at: String,
        failure: Failure,
        steps: Vec<StepReport>,
        residue: Vec<Residual>,
    },
    /// The saga stopped without unwinding, because the chosen rung was retry, resume, rebind,
    /// quarantine, degrade or escalate. Somebody has to act.
    Suspended {
        at: String,
        failure: Failure,
        recovery: Recovery,
        steps: Vec<StepReport>,
    },
}

impl SagaOutcome {
    /// True only for [`SagaOutcome::CompensatedFully`].
    ///
    /// This is the accessor a caller reaches for when it wants to know whether it can forget the
    /// saga happened, and it says yes in exactly one case.
    pub fn restored_prior_state(&self) -> bool {
        matches!(self, SagaOutcome::CompensatedFully { .. })
    }

    /// True when the forward path completed.
    pub fn committed(&self) -> bool {
        matches!(self, SagaOutcome::Committed { .. })
    }

    /// Everything compensation could not undo.
    pub fn residue(&self) -> &[Residual] {
        match self {
            SagaOutcome::CompensatedWithResidue { residue, .. }
            | SagaOutcome::CompensationBlocked { residue, .. }
            | SagaOutcome::PartiallyAccepted { residue, .. } => residue,
            _ => &[],
        }
    }

    /// Residue that somebody still has to do something about.
    pub fn outstanding_remediation(&self) -> Vec<&Residual> {
        self.residue()
            .iter()
            .filter(|residual| !matches!(residual.remediation, Remediation::NotRequired))
            .collect()
    }

    pub fn steps(&self) -> &[StepReport] {
        match self {
            SagaOutcome::Committed { steps }
            | SagaOutcome::CompensatedFully { steps, .. }
            | SagaOutcome::CompensatedWithResidue { steps, .. }
            | SagaOutcome::CompensationBlocked { steps, .. }
            | SagaOutcome::PartiallyAccepted { steps, .. }
            | SagaOutcome::Suspended { steps, .. } => steps,
        }
    }
}
