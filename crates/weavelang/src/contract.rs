//! The cognitive type system: what a well-typed WeaveLang program may not do.
//!
//! Blueprint 23.04. Its thesis is that a type here carries more than data shape — it carries
//! epistemic status, authority, effects and lifecycle — and its conformance list asks for tests on
//! "effect mismatch rejection, capability attenuation, label-flow rejection, linear budget
//! conservation". Those are the four checks in this module, plus the resume-fidelity rule 23.10
//! implies and 23.04 states as "continuations can be moved or forked only if their snapshot
//! fidelity permits".
//!
//! Four rejections are worth more than the rest, and each names a program that must not compile:
//!
//! 1. **An act sent to a participant whose contract does not accept it.** 23.05 makes an act a
//!    typed state transition, so a `challenge` delivered to a participant with no challenge
//!    handler is not a rude message, it is a type error.
//! 2. **A budget consumed twice.** `bioprism_weave::Budget` deliberately does not implement
//!    `Clone`, and a language over it must not reintroduce duplication. The WeaveLang shape of the
//!    violation is two forks from one checkpoint, each drawing a lease: a checkpoint is a snapshot,
//!    so both forks believe they start from the same allowance and the ceiling stops meaning
//!    anything. [`Lease`] is likewise not `Clone`, so the checker cannot make the mistake it is
//!    checking for.
//! 3. **A continuation resumed at a fidelity the participant cannot hold.** A P1 adapter asked to
//!    resume an R3 continuation will silently drop state, and 23.49 puts the grade in the kernel's
//!    `Participant` precisely so nobody has to find that out at run time.
//! 4. **A `Claim<P>` used where `Verified<P>` is required.** 23.04: "A `Claim<P>` cannot satisfy a
//!    requirement for `Verified<P>` without a verifier transition." This is the type-system form of
//!    the workspace rule that a right answer from an incomplete basis is not a pass.
//!
//! Not implemented: refinement checking (23.04's `where provenance.complete and
//! source.independent_count >= 2` needs an evaluator this crate does not have — refinements are
//! parsed nowhere and are listed as unresolved), subtyping over generic type constructors, schema
//! evolution, and adapter loss reporting. Each is named in 23.04's conformance list and each is
//! absent here.

use crate::ast::*;
use crate::diagnostic::{Diagnostic, Span};
use crate::ir::{ContinuationIr, ResumeGrade, SecurityLabel};
use crate::lower::{kernel_act, kernel_resource};
use bioprism_weave::{ActKind, Budget, Resource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TypeError {
    #[error("`{act}` sent to `{recipient}` at {span}, whose contract accepts only {accepted:?}")]
    ActNotAccepted {
        act: String,
        recipient: String,
        accepted: Vec<String>,
        span: Span,
    },

    #[error("checkpoint `{checkpoint}` is leased by fork `{first}` and again by fork `{second}` at {span}; a snapshot's allowance cannot be drawn twice")]
    CheckpointLeasedTwice {
        checkpoint: String,
        first: String,
        second: String,
        span: Span,
    },

    #[error("branch `{branch}` at {span} requests {requested} {resource:?} but only {available} remains in the lease")]
    LeaseExhausted {
        branch: String,
        resource: Resource,
        requested: u64,
        available: u64,
        span: Span,
    },

    #[error("continuation `{continuation}` has fidelity {required:?} but participant `{participant}` holds at most {held:?}")]
    FidelityTooHigh {
        continuation: String,
        participant: String,
        required: ResumeGrade,
        held: ResumeGrade,
    },

    #[error(
        "participant `{participant}` holds ABI grade {held} but role `{role}` needs {required}"
    )]
    AbiGradeTooLow {
        participant: String,
        role: String,
        held: u8,
        required: u8,
    },

    #[error("`{candidate}` cannot substitute for role `{role}`: it carries effects {extra:?} the role never declared")]
    EffectNotSubstitutable {
        candidate: String,
        role: String,
        extra: Vec<String>,
    },

    #[error(
        "a {supplied} cannot satisfy a requirement for {required} without a verifier transition"
    )]
    UnverifiedValue { supplied: String, required: String },

    #[error("`{sender}` labelled {sender_label} sends to `{recipient}` cleared only for {recipient_label} at {span}; no declassification is declared")]
    LabelEscalation {
        sender: String,
        recipient: String,
        sender_label: String,
        recipient_label: String,
        span: Span,
    },

    #[error("`{participant}` at {span} has no contract, so nothing about it can be checked")]
    NoContract { participant: String, span: Span },
}

impl Diagnostic for TypeError {
    fn code(&self) -> &'static str {
        match self {
            TypeError::ActNotAccepted { .. } => "WEAVE-E4001",
            TypeError::CheckpointLeasedTwice { .. } => "WEAVE-E4002",
            TypeError::LeaseExhausted { .. } => "WEAVE-E4003",
            TypeError::FidelityTooHigh { .. } => "WEAVE-E4004",
            TypeError::AbiGradeTooLow { .. } => "WEAVE-E4005",
            TypeError::EffectNotSubstitutable { .. } => "WEAVE-E4006",
            TypeError::UnverifiedValue { .. } => "WEAVE-E4007",
            TypeError::LabelEscalation { .. } => "WEAVE-E4008",
            TypeError::NoContract { .. } => "WEAVE-E4009",
        }
    }

    fn span(&self) -> Option<Span> {
        match self {
            TypeError::ActNotAccepted { span, .. }
            | TypeError::CheckpointLeasedTwice { span, .. }
            | TypeError::LeaseExhausted { span, .. }
            | TypeError::LabelEscalation { span, .. }
            | TypeError::NoContract { span, .. } => Some(*span),
            TypeError::FidelityTooHigh { .. }
            | TypeError::AbiGradeTooLow { .. }
            | TypeError::EffectNotSubstitutable { .. }
            | TypeError::UnverifiedValue { .. } => None,
        }
    }
}

/// 23.04's cognitive types, in the subset the compiler can reason about.
///
/// The proposition is carried as an opaque identifier: the type system's job is to keep `Claim<P>`
/// and `Verified<P>` apart, not to decide whether `P` is true. Deciding that is the kernel's
/// explicit non-goal (23.49) and this crate inherits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "of")]
pub enum CognitiveType {
    Goal(String),
    Question(String),
    Observation(String),
    Claim(String),
    Hypothesis(String),
    Assumption(String),
    Counterexample(String),
    /// The result of a verifier transition, and the only thing that satisfies a `Verified`
    /// requirement.
    Verified(String),
    Plan(String),
    Proposal(String),
    /// 23.04's first-class uncertainty. `Unknown` is not `false`, and none of these four are each
    /// other.
    Unknown {
        reason: String,
    },
    Conflicted {
        candidates: Vec<String>,
    },
    Partial {
        of: String,
        missing: Vec<String>,
    },
    Blocked {
        requirement: String,
    },
}

impl CognitiveType {
    pub fn name(&self) -> &'static str {
        match self {
            CognitiveType::Goal(_) => "Goal",
            CognitiveType::Question(_) => "Question",
            CognitiveType::Observation(_) => "Observation",
            CognitiveType::Claim(_) => "Claim",
            CognitiveType::Hypothesis(_) => "Hypothesis",
            CognitiveType::Assumption(_) => "Assumption",
            CognitiveType::Counterexample(_) => "Counterexample",
            CognitiveType::Verified(_) => "Verified",
            CognitiveType::Plan(_) => "Plan",
            CognitiveType::Proposal(_) => "Proposal",
            CognitiveType::Unknown { .. } => "Unknown",
            CognitiveType::Conflicted { .. } => "Conflicted",
            CognitiveType::Partial { .. } => "Partial",
            CognitiveType::Blocked { .. } => "Blocked",
        }
    }

    /// The proposition a cognitive value is about, where it has one.
    pub fn proposition(&self) -> Option<&str> {
        match self {
            CognitiveType::Goal(p)
            | CognitiveType::Question(p)
            | CognitiveType::Observation(p)
            | CognitiveType::Claim(p)
            | CognitiveType::Hypothesis(p)
            | CognitiveType::Assumption(p)
            | CognitiveType::Counterexample(p)
            | CognitiveType::Verified(p)
            | CognitiveType::Plan(p)
            | CognitiveType::Proposal(p) => Some(p),
            CognitiveType::Partial { of, .. } => Some(of),
            CognitiveType::Unknown { .. }
            | CognitiveType::Conflicted { .. }
            | CognitiveType::Blocked { .. } => None,
        }
    }

    /// Whether a value of this type satisfies a requirement for `required`.
    ///
    /// Identity apart, the only widening allowed is that an `Observation` about a proposition
    /// satisfies a `Claim` about it — observing something is asserting it. Everything else is
    /// distinct, and in particular a `Claim` never satisfies a `Verified`: promoting one to the
    /// other is a verifier's job, and the type system's whole purpose here is to make sure some
    /// transition actually did it.
    pub fn satisfies(&self, required: &CognitiveType) -> bool {
        if self == required {
            return true;
        }
        matches!(
            (self, required),
            (CognitiveType::Observation(a), CognitiveType::Claim(b))
                | (CognitiveType::Verified(a), CognitiveType::Claim(b)) if a == b
        )
    }

    /// The typed rejection, for a caller that wants the error rather than a boolean.
    pub fn check_satisfies(&self, required: &CognitiveType) -> Result<(), TypeError> {
        if self.satisfies(required) {
            Ok(())
        } else {
            Err(TypeError::UnverifiedValue {
                supplied: self.name().to_string(),
                required: required.name().to_string(),
            })
        }
    }
}

/// What a runtime participant will accept, provide and hold.
///
/// 23.02 binds participants at run time, so a contract is supplied to the checker rather than read
/// out of the source. That is the point: the same program is well-typed against one roster and
/// ill-typed against another, and the compiler can say which before anything runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantContract {
    pub id: String,
    pub role: String,
    /// Acts this participant has a handler for.
    pub accepts: BTreeSet<ActKind>,
    pub provides: BTreeSet<String>,
    pub effects: BTreeSet<String>,
    pub clearance: SecurityLabel,
    /// The highest continuation grade this participant can hold (23.49's ABI grade).
    pub max_resume_grade: ResumeGrade,
}

impl ParticipantContract {
    pub fn new(id: impl Into<String>, role: impl Into<String>) -> Self {
        ParticipantContract {
            id: id.into(),
            role: role.into(),
            accepts: BTreeSet::new(),
            provides: BTreeSet::new(),
            effects: BTreeSet::new(),
            clearance: SecurityLabel::new("public"),
            max_resume_grade: ResumeGrade::R1,
        }
    }

    pub fn accepting(mut self, acts: impl IntoIterator<Item = ActKind>) -> Self {
        self.accepts.extend(acts);
        self
    }

    pub fn with_effects<S: Into<String>>(mut self, effects: impl IntoIterator<Item = S>) -> Self {
        self.effects.extend(effects.into_iter().map(Into::into));
        self
    }

    pub fn cleared_for(mut self, level: impl Into<String>) -> Self {
        self.clearance = SecurityLabel::new(level);
        self
    }

    pub fn resuming(mut self, grade: ResumeGrade) -> Self {
        self.max_resume_grade = grade;
        self
    }

    /// Whether this participant may resume a continuation.
    pub fn check_resume(&self, continuation: &ContinuationIr) -> Result<(), TypeError> {
        if continuation.fidelity <= self.max_resume_grade {
            Ok(())
        } else {
            Err(TypeError::FidelityTooHigh {
                continuation: continuation.continuation_id.clone(),
                participant: self.id.clone(),
                required: continuation.fidelity,
                held: self.max_resume_grade,
            })
        }
    }

    /// Whether this participant may stand in for a role.
    ///
    /// 23.04: "An agent with a compatible return type but broader undeclared effects cannot be
    /// substituted." Broader is the failure; narrower is fine.
    pub fn check_substitutable(&self, role: &RoleDecl) -> Result<(), TypeError> {
        let declared: BTreeSet<String> = role.requires.iter().map(Path::text).collect();
        let extra: Vec<String> = self.effects.difference(&declared).cloned().collect();
        if extra.is_empty() {
            Ok(())
        } else {
            Err(TypeError::EffectNotSubstitutable {
                candidate: self.id.clone(),
                role: role.name.clone(),
                extra,
            })
        }
    }
}

/// A drawn allowance.
///
/// Not `Clone` and not `Copy`, for the same reason `bioprism_weave::Budget` is not: the checker
/// that enforces non-duplication must not be able to duplicate. Consuming one takes it by value.
#[derive(Debug, PartialEq, Eq)]
pub struct Lease {
    checkpoint: String,
    branch: String,
    resource: Resource,
    amount: u64,
}

impl Lease {
    pub fn checkpoint(&self) -> &str {
        &self.checkpoint
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn resource(&self) -> Resource {
        self.resource
    }

    pub fn amount(&self) -> u64 {
        self.amount
    }
}

/// What a successful check found, for a caller that wants more than "it type-checked".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeReport {
    /// Checkpoints that had a lease drawn against them, and by which branches.
    pub leased_checkpoints: BTreeMap<String, Vec<String>>,
    /// Acts checked against a recipient contract.
    pub checked_acts: usize,
    /// Participants named by the program that no contract covered. Reported rather than assumed
    /// safe: an unchecked participant is not a checked one.
    pub unchecked_participants: BTreeSet<String>,
}

/// Type-checks a program against a roster of participant contracts.
pub fn check(
    program: &Program,
    contracts: &BTreeMap<String, ParticipantContract>,
) -> Result<TypeReport, TypeError> {
    let roles: BTreeMap<&str, &RoleDecl> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Role(decl) => Some((decl.name.as_str(), decl)),
            _ => None,
        })
        .collect();
    let policy_ceiling = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Policy(decl) => Some(ceiling_of(decl)),
            _ => None,
        })
        .unwrap_or_default();

    let mut checker = Checker {
        contracts,
        roles: &roles,
        report: TypeReport::default(),
        ceiling: policy_ceiling,
        fork_of_checkpoint: BTreeMap::new(),
    };

    for item in &program.items {
        if let Item::Weave(weave) = item {
            checker.check_block(&weave.body)?;
        }
    }
    Ok(checker.report)
}

fn ceiling_of(policy: &PolicyDecl) -> Budget {
    let mut budget = Budget::new();
    for limit in &policy.budgets {
        if let (Some(resource), Literal::Integer(value)) =
            (kernel_resource(&limit.resource), &limit.limit)
        {
            budget = budget.with(resource, (*value).max(0) as u64);
        }
    }
    budget
}

struct Checker<'a> {
    contracts: &'a BTreeMap<String, ParticipantContract>,
    roles: &'a BTreeMap<&'a str, &'a RoleDecl>,
    report: TypeReport,
    ceiling: Budget,
    fork_of_checkpoint: BTreeMap<String, String>,
}

impl Checker<'_> {
    fn check_block(&mut self, body: &[Stmt]) -> Result<(), TypeError> {
        for statement in body {
            self.check_statement(statement)?;
        }
        Ok(())
    }

    fn check_statement(&mut self, statement: &Stmt) -> Result<(), TypeError> {
        match statement {
            Stmt::Bind { name, role, span } => {
                if let Some(contract) = self.contracts.get(name) {
                    if let Some(declaration) = self.roles.get(role.as_str()) {
                        contract.check_substitutable(declaration)?;
                    }
                } else {
                    self.report.unchecked_participants.insert(name.clone());
                    let _ = span;
                }
            }
            Stmt::Send {
                act,
                from,
                to,
                span,
                ..
            } => {
                self.check_act(act, from, to, *span)?;
            }
            Stmt::Fork {
                from,
                branches,
                span,
            } => {
                let leases = self.draw_leases(from, branches, *span)?;
                for lease in leases {
                    // The lease is consumed here, by value. A second consumption is not an error
                    // this code can report, because it is not code this crate can write.
                    self.report
                        .leased_checkpoints
                        .entry(lease.checkpoint().to_string())
                        .or_default()
                        .push(lease.branch().to_string());
                }
                for branch in branches {
                    self.check_block(&branch.body)?;
                }
            }
            Stmt::Race { branches, .. } => {
                for branch in branches {
                    self.check_block(&branch.body)?;
                }
            }
            Stmt::Par { body, .. } | Stmt::Repeat { body, .. } => self.check_block(body)?,
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    self.check_block(&arm.body)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn check_act(&mut self, act: &str, from: &str, to: &str, span: Span) -> Result<(), TypeError> {
        let Some(kind) = kernel_act(act) else {
            return Ok(());
        };
        let Some(recipient) = self.contracts.get(to) else {
            self.report.unchecked_participants.insert(to.to_string());
            return Ok(());
        };
        self.report.checked_acts += 1;

        if !recipient.accepts.contains(&kind) {
            return Err(TypeError::ActNotAccepted {
                act: act.to_string(),
                recipient: to.to_string(),
                accepted: recipient
                    .accepts
                    .iter()
                    .map(|accepted| accepted.as_str().to_string())
                    .collect(),
                span,
            });
        }

        if let Some(sender) = self.contracts.get(from) {
            if !recipient.clearance.dominates(&sender.clearance) {
                return Err(TypeError::LabelEscalation {
                    sender: from.to_string(),
                    recipient: to.to_string(),
                    sender_label: sender.clearance.level.clone(),
                    recipient_label: recipient.clearance.level.clone(),
                    span,
                });
            }
        }
        Ok(())
    }

    /// Draws every branch lease of one fork out of the shared ceiling.
    ///
    /// A checkpoint may back at most one fork's leases. Two forks from one checkpoint would each
    /// start from the same recorded allowance, which is duplication however carefully each one is
    /// bounded on its own.
    fn draw_leases(
        &mut self,
        checkpoint: &str,
        branches: &[Branch],
        span: Span,
    ) -> Result<Vec<Lease>, TypeError> {
        let leasing = branches.iter().any(|branch| !branch.budget.is_empty());
        if leasing {
            if let Some(first) = self.fork_of_checkpoint.get(checkpoint) {
                return Err(TypeError::CheckpointLeasedTwice {
                    checkpoint: checkpoint.to_string(),
                    first: first.clone(),
                    second: branches
                        .first()
                        .map(|branch| branch.name.clone())
                        .unwrap_or_default(),
                    span,
                });
            }
            self.fork_of_checkpoint.insert(
                checkpoint.to_string(),
                branches
                    .first()
                    .map(|branch| branch.name.clone())
                    .unwrap_or_default(),
            );
        }

        let mut leases = Vec::new();
        for branch in branches {
            for grant in &branch.budget {
                let Some(resource) = kernel_resource(&grant.resource) else {
                    continue;
                };
                let available = self.ceiling.remaining(resource);
                let drawn = self.ceiling.split(resource, grant.amount).map_err(|_| {
                    TypeError::LeaseExhausted {
                        branch: branch.name.clone(),
                        resource,
                        requested: grant.amount,
                        available,
                        span: grant.span,
                    }
                })?;
                debug_assert_eq!(drawn.remaining(resource), grant.amount);
                leases.push(Lease {
                    checkpoint: checkpoint.to_string(),
                    branch: branch.name.clone(),
                    resource,
                    amount: grant.amount,
                });
            }
        }
        Ok(leases)
    }
}

/// Checks that every participant filling a role can hold the continuations the program needs.
pub fn check_abi_grades(
    required_grade: u8,
    contracts: &BTreeMap<String, ParticipantContract>,
) -> Result<(), TypeError> {
    for contract in contracts.values() {
        let held = contract.max_resume_grade.as_grade();
        if held < required_grade {
            return Err(TypeError::AbiGradeTooLow {
                participant: contract.id.clone(),
                role: contract.role.clone(),
                held,
                required: required_grade,
            });
        }
    }
    Ok(())
}
