//! Humans and organizations as first-class participants: requests, capsules, attention, approvals.
//!
//! Blueprint 23.47.
//!
//! # The one thing this module is for
//!
//! 23.47: **"Human approval is not a magical bypass."** [`Approval`] therefore records what the
//! human was authorized to approve, what evidence was visible, and what scope the resulting grant
//! has, and [`Approval::authorizes`] refuses an action outside that scope. An approval is a
//! bounded, typed grant of a `bioprism_weave::Capability`, not a flag that turns a refusal into a
//! permission.
//!
//! The scope check is only half of it. 23.47's first listed failure mode is "rubber-stamp approval
//! caused by poor context", and that one is decidable here because the capsule the human read is
//! the kernel's own `bioprism_weave::ContextCapsule`, which already reports what the projection
//! withheld. [`DecisionCapsule::basis`] therefore distinguishes an approval given on complete
//! evidence from one given on a capsule that withheld items, and the distinction survives into the
//! [`Approval`] rather than being recomputed later from a transcript nobody kept.
//!
//! # Attention is a budget, and the kernel has no word for it
//!
//! 23.47 asks the scheduler to track interruption cost, review duration, queue, expertise, fatigue,
//! urgency and reuse. 23.16's resource vocabulary — which `bioprism_weave::Budget` accounts — has
//! `Tokens`, `ToolCalls` and `WallClockMillis` and nothing for a person's time. Adding a variant to
//! the kernel would grow a trusted computing base whose size is a design constraint, so
//! [`Reviewer`] holds a real `Budget` denominated in `Resource::WallClockMillis` and this sentence
//! says what that costs: **a thread's elapsed milliseconds and a reviewer's attention milliseconds
//! are different quantities that share a unit here.** What the reuse buys is the affine rule for
//! free — `Budget` is not `Clone`, so a reviewer's attention cannot be promised to two threads at
//! once, and [`Reviewer`] is not `Clone` for the same reason.
//!
//! # Organizational obligations outlive sessions
//!
//! 23.47: "Organizational obligations outlive individual agent sessions and require durable
//! identity and succession rules." [`OrganizationalCommitment::end_session`] does not discharge the
//! commitment; it returns the same commitment with a new responsible party drawn from the
//! succession, and refuses when the succession is empty. There is no method that ends a commitment
//! because a session ended.
//!
//! # Not implemented
//!
//! No scheduling, no queueing, no notification, no interface. Nothing here contacts a human; the
//! crate performs no effect. Of 23.47's eight failure modes, five are decidable from the records
//! this module keeps and are implemented as [`FailureMode::detectable`]; the other three —
//! ambiguous responsibility, agent-generated consensus suppressing dissent, and human intervention
//! used as an unmeasured benchmark escape hatch — are properties of a programme rather than of a
//! record and are named without detectors. 23.47's evaluation section belongs to WeaveBench and is
//! recorded in [`EVALUATION_MEASURES`].

use bioprism_fabric::effect::Irreversibility;
use bioprism_section::Layer;
use bioprism_weave::{Budget, BudgetError, Capability, ContextCapsule, Resource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.47's seven participant kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantKind {
    IndividualHuman,
    OnCallHumanPool,
    ExpertPanel,
    OrganizationalRole,
    InstitutionalAuthority,
    MixedHumanAgentMolecule,
    ExternalAccountableParty,
}

impl ParticipantKind {
    pub const ALL: [ParticipantKind; 7] = [
        ParticipantKind::IndividualHuman,
        ParticipantKind::OnCallHumanPool,
        ParticipantKind::ExpertPanel,
        ParticipantKind::OrganizationalRole,
        ParticipantKind::InstitutionalAuthority,
        ParticipantKind::MixedHumanAgentMolecule,
        ParticipantKind::ExternalAccountableParty,
    ];

    /// Whether a human judgement is guaranteed to be in the loop for this kind.
    ///
    /// [`ParticipantKind::MixedHumanAgentMolecule`] is deliberately `false`: a mixed molecule may
    /// route a given decision entirely to its agent members, and calling its output
    /// human-reviewed is 23.47's "hidden automation presented as human-reviewed".
    pub fn guarantees_human_judgement(self) -> bool {
        matches!(
            self,
            ParticipantKind::IndividualHuman
                | ParticipantKind::OnCallHumanPool
                | ParticipantKind::ExpertPanel
        )
    }
}

/// What is being asked of the human.
///
/// A specific decision or a specific artifact, never a topic. This is where 23.47's "a vague
/// 'please review everything' request fails conformance" is enforced: there is no variant that
/// takes a subject area, and [`Requested::subject`] is a required field on both variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "requested")]
pub enum Requested {
    Decision { subject: String, question: String },
    Artifact { subject: String, artifact_id: String },
}

impl Requested {
    pub fn subject(&self) -> &str {
        match self {
            Requested::Decision { subject, .. } | Requested::Artifact { subject, .. } => subject,
        }
    }
}

/// What happens if the human does nothing.
///
/// Not an `Option<String>`. 23.47 requires "consequences of approval, rejection, **or no
/// response**", and a request that cannot say what silence means has not stated its own stakes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consequences {
    pub of_approval: String,
    pub of_rejection: String,
    pub of_no_response: String,
}

/// 23.47's ten-part human-facing contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanRequest {
    pub requested: Requested,
    /// "why human judgment is required".
    pub why_human: String,
    /// "exact decision rights".
    pub decision_rights: Capability,
    /// "deadline and urgency", as a logical deadline a caller supplies. This crate has no clock.
    pub deadline_tick: u64,
    pub urgency: Urgency,
    /// "estimated attention cost", in minutes.
    pub estimated_minutes: u32,
    /// "evidence and uncertainty".
    pub uncertainty: Vec<String>,
    /// "alternatives considered".
    pub alternatives: Vec<String>,
    pub consequences: Consequences,
    /// "whether the action is reversible", in 23.14's classes.
    pub reversibility: Irreversibility,
    /// "how dissent will be recorded".
    pub dissent_recording: String,
}

/// How urgent the request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Routine,
    Elevated,
    Immediate,
}

/// A way a human request fails 23.47's conformance requirement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "defect")]
pub enum RequestDefect {
    #[error("the request states no subject")]
    NoSubject,

    #[error("the request does not say why human judgement is required")]
    NoJustification,

    #[error("the request states no attention estimate")]
    NoAttentionEstimate,

    #[error("the request considers no alternatives")]
    NoAlternatives,

    #[error("the request does not say what happens if nobody responds")]
    NoSilenceConsequence,

    #[error("the request does not say how dissent will be recorded")]
    NoDissentRecording,

    #[error("the request is unbounded: {subject}")]
    UnboundedScope { subject: String },
}

/// Subjects that make a request unbounded rather than specific.
///
/// 23.47 names exactly one example — "please review everything" — so the test is a small, literal
/// list rather than a heuristic. A heuristic here would be worse than nothing: it would make
/// conformance depend on wording rather than on scope, and it would let a genuinely unbounded
/// request through by rephrasing.
const UNBOUNDED_SUBJECTS: [&str; 4] = ["everything", "all of it", "anything", "the whole thing"];

impl HumanRequest {
    /// 23.47's conformance check over the ten-part contract.
    ///
    /// Returns every defect. `estimated_minutes == 0` counts as no estimate, because a review
    /// costing no attention is not a review.
    pub fn defects(&self) -> BTreeSet<RequestDefect> {
        let mut defects = BTreeSet::new();
        let subject = self.requested.subject().trim().to_ascii_lowercase();
        if subject.is_empty() {
            defects.insert(RequestDefect::NoSubject);
        } else if UNBOUNDED_SUBJECTS.contains(&subject.as_str()) {
            defects.insert(RequestDefect::UnboundedScope { subject });
        }
        if self.why_human.trim().is_empty() {
            defects.insert(RequestDefect::NoJustification);
        }
        if self.estimated_minutes == 0 {
            defects.insert(RequestDefect::NoAttentionEstimate);
        }
        if self.alternatives.is_empty() {
            defects.insert(RequestDefect::NoAlternatives);
        }
        if self.consequences.of_no_response.trim().is_empty() {
            defects.insert(RequestDefect::NoSilenceConsequence);
        }
        if self.dissent_recording.trim().is_empty() {
            defects.insert(RequestDefect::NoDissentRecording);
        }
        defects
    }

    pub fn conformant(&self) -> bool {
        self.defects().is_empty()
    }
}

/// The options 23.47's Decision Capsule offers, from its worked example.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewOption {
    Approve,
    Reject,
    RequestTargetedEvidence,
    DelegateToOwner,
}

impl ReviewOption {
    pub const ALL: [ReviewOption; 4] = [
        ReviewOption::Approve,
        ReviewOption::Reject,
        ReviewOption::RequestTargetedEvidence,
        ReviewOption::DelegateToOwner,
    ];
}

/// What the human's judgement rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "basis")]
pub enum Basis {
    /// The projection withheld nothing and the compiler's own omissions still support a
    /// sufficiency claim.
    CompleteEvidence,
    /// The projection withheld items, named. An approval on this basis is still an approval; what
    /// it is not is an informed one, and the record says which.
    PartialEvidence { withheld: Vec<String> },
    /// The projection withheld nothing but the upstream compilation did not support a sufficiency
    /// claim. Distinct from the case above: the human saw everything that was selected, and what
    /// was selected may not have been enough.
    UpstreamInsufficient,
}

impl Basis {
    pub fn informed(&self) -> bool {
        matches!(self, Basis::CompleteEvidence)
    }
}

/// 23.47's specialized Context Capsule for a human.
///
/// Built *around* the kernel's `ContextCapsule` rather than beside it, so the withholding record
/// and the sufficiency verdict come from the projection that actually happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionCapsule {
    pub objective: String,
    pub recommended_action: ReviewOption,
    pub blocking_issue: String,
    pub supporting: Vec<String>,
    pub opposing: Vec<String>,
    pub unresolved: Vec<String>,
    pub authority_requested: Capability,
    pub reversibility: Irreversibility,
    pub estimated_review_minutes: u32,
    pub options: BTreeSet<ReviewOption>,
    capsule: ContextCapsule,
}

/// Why a decision capsule could not be built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "defect")]
pub enum CapsuleDefect {
    #[error("a decision capsule must offer at least two options")]
    TooFewOptions,

    #[error("the recommended action {0:?} is not among the offered options")]
    RecommendationNotOffered(ReviewOption),

    #[error("a class {class:?} action needs at least layer {required:?}, capsule is at {actual:?}")]
    LayerTooShallow {
        class: Irreversibility,
        required: Layer,
        actual: Layer,
    },
}

/// The evidence layer a decision at this irreversibility class must be shown at.
///
/// **Not in the blueprint.** 23.47 gives a capsule with `reversibility: low` and no rule relating
/// reversibility to how much evidence the human must be shown, while `bioprism-section`'s layers
/// are exactly a depth-of-evidence ladder. The mapping is this crate's reading and is deliberately
/// coarse: everything below E3 may be decided from the decision evidence itself (`L2`), an
/// externally visible or costly action needs the computed views and provenance behind it (`L3`),
/// and an irreversible one needs the governed raw artifacts (`L4`).
pub fn required_layer(class: Irreversibility) -> Layer {
    match class {
        Irreversibility::E0 | Irreversibility::E1 | Irreversibility::E2 => Layer::L2,
        Irreversibility::E3 => Layer::L3,
        Irreversibility::E4 => Layer::L4,
    }
}

/// A capsule under construction. [`Draft::build`] is the only route to a [`DecisionCapsule`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    objective: String,
    blocking_issue: String,
    recommended_action: ReviewOption,
    authority_requested: Capability,
    reversibility: Irreversibility,
    estimated_review_minutes: u32,
    options: BTreeSet<ReviewOption>,
    capsule: ContextCapsule,
}

impl Draft {
    pub fn offering(mut self, option: ReviewOption) -> Self {
        self.options.insert(option);
        self
    }

    /// Set the recommendation. It is deliberately *not* added to the offered options: a capsule
    /// recommending something it does not offer is a defect [`Draft::build`] catches, and a setter
    /// that quietly fixed it would make the defect unreachable.
    pub fn recommending(mut self, option: ReviewOption) -> Self {
        self.recommended_action = option;
        self
    }

    pub fn requesting(mut self, capability: Capability) -> Self {
        self.authority_requested = capability;
        self
    }

    pub fn at_class(mut self, reversibility: Irreversibility) -> Self {
        self.reversibility = reversibility;
        self
    }

    pub fn estimated_minutes(mut self, minutes: u32) -> Self {
        self.estimated_review_minutes = minutes;
        self
    }

    /// Check the three structural rules and produce the capsule.
    ///
    /// The options rule is read from 23.47's worked example: it offers four options, and a
    /// "capsule" with one option is a notification. Requiring two is the weakest form of that rule
    /// which is still a rule.
    pub fn build(self) -> Result<DecisionCapsule, CapsuleDefect> {
        if self.options.len() < 2 {
            return Err(CapsuleDefect::TooFewOptions);
        }
        if !self.options.contains(&self.recommended_action) {
            return Err(CapsuleDefect::RecommendationNotOffered(
                self.recommended_action,
            ));
        }
        let required = required_layer(self.reversibility);
        if self.capsule.layer < required {
            return Err(CapsuleDefect::LayerTooShallow {
                class: self.reversibility,
                required,
                actual: self.capsule.layer,
            });
        }
        Ok(DecisionCapsule {
            objective: self.objective,
            recommended_action: self.recommended_action,
            blocking_issue: self.blocking_issue,
            supporting: Vec::new(),
            opposing: Vec::new(),
            unresolved: Vec::new(),
            authority_requested: self.authority_requested,
            reversibility: self.reversibility,
            estimated_review_minutes: self.estimated_review_minutes,
            options: self.options,
            capsule: self.capsule,
        })
    }
}

impl DecisionCapsule {
    /// Start a capsule around a kernel projection.
    ///
    /// The defaults are the safe ones: reject as the recommendation, the weakest capability, the
    /// most irreversible class. A caller that forgets to state the class gets the strictest layer
    /// requirement rather than the loosest.
    pub fn draft(
        objective: impl Into<String>,
        blocking_issue: impl Into<String>,
        capsule: ContextCapsule,
    ) -> Draft {
        Draft {
            objective: objective.into(),
            blocking_issue: blocking_issue.into(),
            recommended_action: ReviewOption::Reject,
            authority_requested: Capability::ReadWorld,
            reversibility: Irreversibility::E4,
            estimated_review_minutes: 0,
            options: BTreeSet::new(),
            capsule,
        }
    }

    pub fn supported_by(mut self, evidence: impl Into<String>) -> Self {
        self.supporting.push(evidence.into());
        self
    }

    pub fn opposed_by(mut self, evidence: impl Into<String>) -> Self {
        self.opposing.push(evidence.into());
        self
    }

    pub fn unresolved(mut self, question: impl Into<String>) -> Self {
        self.unresolved.push(question.into());
        self
    }

    /// The underlying kernel projection, for a reader who wants the provenance the capsule links to.
    pub fn context(&self) -> &ContextCapsule {
        &self.capsule
    }

    /// What a decision taken from this capsule would rest on.
    ///
    /// Reads the kernel capsule rather than a separate record, so it cannot disagree with the
    /// projection that produced it.
    pub fn basis(&self) -> Basis {
        if !self.capsule.is_complete() {
            Basis::PartialEvidence {
                withheld: self
                    .capsule
                    .withheld
                    .iter()
                    .map(|item| item.id.clone())
                    .collect(),
            }
        } else if !self.capsule.upstream_supports_sufficiency {
            Basis::UpstreamInsufficient
        } else {
            Basis::CompleteEvidence
        }
    }
}

/// A human reviewer with a finite attention allowance.
///
/// Not `Clone`, because it owns a `Budget`, which is not `Clone`. That is the affine rule of 23.16
/// arriving here without being reimplemented: a reviewer's remaining attention cannot be handed to
/// two threads at once.
///
/// `Serialize` and not `Deserialize`, for the same reason one step further out: a reviewer that
/// could be read back from bytes could be read back twice, and the second copy would arrive with
/// the attention the first one already spent.
#[derive(Debug, Serialize)]
pub struct Reviewer {
    pub name: String,
    pub kind: ParticipantKind,
    pub expertise: BTreeSet<String>,
    /// 23.47's "fatigue and repeated-request limits": how many requests this reviewer will take
    /// before further ones are refused as overload.
    pub request_limit: u32,
    requests_taken: u32,
    attention: Budget,
}

/// Why a review request was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum ReviewRefusal {
    #[error("{reviewer} lacks the required expertise {required}")]
    MissingExpertise { reviewer: String, required: String },

    #[error("{reviewer} has taken {taken} requests against a limit of {limit}")]
    Overloaded {
        reviewer: String,
        taken: u32,
        limit: u32,
    },

    /// The kernel's `BudgetError` is rendered into minutes here rather than carried, because
    /// `bioprism_weave::BudgetError` is not serialisable and every public type in this crate is.
    /// Nothing is lost: the kernel error's two numbers are both present.
    #[error("{reviewer} has {available_minutes} minutes of attention and the request needs {requested_minutes}")]
    AttentionExhausted {
        reviewer: String,
        requested_minutes: u64,
        available_minutes: u64,
    },
}

/// One minute of a reviewer's attention, in the units `Resource::WallClockMillis` counts.
pub const MILLIS_PER_MINUTE: u64 = 60_000;

impl Reviewer {
    /// A reviewer with `available_minutes` of attention.
    pub fn new(name: impl Into<String>, kind: ParticipantKind, available_minutes: u64) -> Self {
        Reviewer {
            name: name.into(),
            kind,
            expertise: BTreeSet::new(),
            request_limit: u32::MAX,
            requests_taken: 0,
            attention: Budget::new()
                .with(Resource::WallClockMillis, available_minutes * MILLIS_PER_MINUTE),
        }
    }

    pub fn expert_in(mut self, area: impl Into<String>) -> Self {
        self.expertise.insert(area.into());
        self
    }

    pub fn limited_to(mut self, requests: u32) -> Self {
        self.request_limit = requests;
        self
    }

    pub fn remaining_minutes(&self) -> u64 {
        self.attention.remaining(Resource::WallClockMillis) / MILLIS_PER_MINUTE
    }

    pub fn requests_taken(&self) -> u32 {
        self.requests_taken
    }

    /// Accept a request, spending the estimated attention.
    ///
    /// Three refusals in 23.47's order of cheapness: expertise, then fatigue, then the budget.
    /// Nothing is spent unless all three pass, so a refused request costs the reviewer nothing —
    /// which is the property that makes repeated misrouting visible as overload rather than as
    /// exhaustion.
    pub fn accept(
        &mut self,
        request: &HumanRequest,
        required_expertise: Option<&str>,
    ) -> Result<(), ReviewRefusal> {
        if let Some(area) = required_expertise {
            if !self.expertise.contains(area) {
                return Err(ReviewRefusal::MissingExpertise {
                    reviewer: self.name.clone(),
                    required: area.to_string(),
                });
            }
        }
        if self.requests_taken >= self.request_limit {
            return Err(ReviewRefusal::Overloaded {
                reviewer: self.name.clone(),
                taken: self.requests_taken,
                limit: self.request_limit,
            });
        }
        let requested_minutes = u64::from(request.estimated_minutes);
        self.attention
            .spend(
                Resource::WallClockMillis,
                requested_minutes * MILLIS_PER_MINUTE,
            )
            .map_err(|error| ReviewRefusal::AttentionExhausted {
                reviewer: self.name.clone(),
                requested_minutes,
                available_minutes: match error {
                    BudgetError::Exhausted { available, .. } => available / MILLIS_PER_MINUTE,
                    BudgetError::Unallocated(_) => 0,
                },
            })?;
        self.requests_taken += 1;
        Ok(())
    }
}

/// A recorded human approval, with everything 23.47's authority-boundary section requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub approver: String,
    pub approver_kind: ParticipantKind,
    /// "what the human was authorized to approve".
    pub granted: Capability,
    /// "what evidence was visible".
    pub basis: Basis,
    /// "whether conflicts were disclosed".
    pub conflicts_disclosed: bool,
    /// "whether consent was informed".
    pub consent_informed: bool,
    /// "what scope and duration the resulting grant has", as a tick count a caller supplies.
    pub duration_ticks: u64,
    /// "whether another institutional approval remains necessary".
    pub further_approval_required: Vec<String>,
}

/// Why an approval does not authorize an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum ApprovalRefusal {
    #[error("{approver} was granted {granted:?}, not {attempted:?}")]
    OutsideScope {
        approver: String,
        granted: Capability,
        attempted: Capability,
    },

    #[error("the grant expired at tick {expires_at}, action is at {now}")]
    Expired { expires_at: u64, now: u64 },

    #[error("a further institutional approval is still required: {outstanding:?}")]
    FurtherApprovalRequired { outstanding: Vec<String> },

    #[error("{approver} is a {kind:?}, which does not guarantee human judgement")]
    NotAHumanJudgement {
        approver: String,
        kind: ParticipantKind,
    },
}

impl Approval {
    /// Whether this approval authorizes a specific action at a specific time.
    ///
    /// Four refusals, and the ordering matters: scope first, because a mismatched capability is a
    /// different failure from an expired one and should be reported as itself. The last clause is
    /// 23.47's "hidden automation presented as human-reviewed" — an approval attributed to a
    /// participant kind that does not guarantee a human judgement does not carry one.
    pub fn authorizes(
        &self,
        attempted: Capability,
        granted_at: u64,
        now: u64,
    ) -> Result<(), ApprovalRefusal> {
        if self.granted != attempted {
            return Err(ApprovalRefusal::OutsideScope {
                approver: self.approver.clone(),
                granted: self.granted.clone(),
                attempted,
            });
        }
        if !self.approver_kind.guarantees_human_judgement() {
            return Err(ApprovalRefusal::NotAHumanJudgement {
                approver: self.approver.clone(),
                kind: self.approver_kind,
            });
        }
        if !self.further_approval_required.is_empty() {
            return Err(ApprovalRefusal::FurtherApprovalRequired {
                outstanding: self.further_approval_required.clone(),
            });
        }
        let expires_at = granted_at.saturating_add(self.duration_ticks);
        if now > expires_at {
            return Err(ApprovalRefusal::Expired { expires_at, now });
        }
        Ok(())
    }
}

/// 23.47's eight failure modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    RubberStampFromPoorContext,
    RepeatedEscalationOverload,
    HiddenAutomationPresentedAsHumanReviewed,
    AmbiguousResponsibility,
    ApprovalRequestedAfterIrreversibleAction,
    OrganizationalAuthorityInferredFromEmail,
    AgentConsensusSuppressingExpertDissent,
    HumanInterventionAsUnmeasuredEscapeHatch,
}

impl FailureMode {
    pub const ALL: [FailureMode; 8] = [
        FailureMode::RubberStampFromPoorContext,
        FailureMode::RepeatedEscalationOverload,
        FailureMode::HiddenAutomationPresentedAsHumanReviewed,
        FailureMode::AmbiguousResponsibility,
        FailureMode::ApprovalRequestedAfterIrreversibleAction,
        FailureMode::OrganizationalAuthorityInferredFromEmail,
        FailureMode::AgentConsensusSuppressingExpertDissent,
        FailureMode::HumanInterventionAsUnmeasuredEscapeHatch,
    ];

    /// Whether this crate can decide the failure mode from the records it keeps.
    ///
    /// Five can. The three that cannot are properties of a programme rather than of a record:
    /// ambiguous responsibility is a question about an org chart, suppressed dissent needs the
    /// counterfactual where the expert spoke, and "unmeasured escape hatch" is a claim about what
    /// a benchmark reported, which lives in `crate::packs`.
    pub fn detectable(self) -> bool {
        matches!(
            self,
            FailureMode::RubberStampFromPoorContext
                | FailureMode::RepeatedEscalationOverload
                | FailureMode::HiddenAutomationPresentedAsHumanReviewed
                | FailureMode::ApprovalRequestedAfterIrreversibleAction
                | FailureMode::OrganizationalAuthorityInferredFromEmail
        )
    }
}

/// One step in a recorded interaction, for after-the-fact failure detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "step")]
pub enum InteractionStep {
    ActionPerformed { class: Irreversibility },
    ApprovalRequested { class: Irreversibility },
    ApprovalGiven { basis: Basis, minutes_spent: u32 },
}

/// Detect the failure modes decidable from an interaction record.
///
/// Deterministic and order-sensitive, which is what makes
/// [`FailureMode::ApprovalRequestedAfterIrreversibleAction`] decidable at all: the failure is
/// entirely in the ordering, not in either event.
///
/// The rubber-stamp rule uses the two facts the record carries — the basis the human decided on and
/// the minutes they spent. An approval on partial evidence, or one given in less than a minute, is
/// flagged. Neither is proof; both are what 23.47 asks a system to notice.
pub fn detect(steps: &[InteractionStep], reviewer_overloaded: bool) -> BTreeSet<FailureMode> {
    let mut found = BTreeSet::new();
    let mut irreversible_done = false;
    for step in steps {
        match step {
            InteractionStep::ActionPerformed { class } => {
                if class.is_irreversible() {
                    irreversible_done = true;
                }
            }
            InteractionStep::ApprovalRequested { class } => {
                if irreversible_done && class.is_irreversible() {
                    found.insert(FailureMode::ApprovalRequestedAfterIrreversibleAction);
                }
            }
            InteractionStep::ApprovalGiven {
                basis,
                minutes_spent,
            } => {
                if !basis.informed() || *minutes_spent == 0 {
                    found.insert(FailureMode::RubberStampFromPoorContext);
                }
            }
        }
    }
    if reviewer_overloaded {
        found.insert(FailureMode::RepeatedEscalationOverload);
    }
    found
}

/// How an organization's identity is established.
///
/// There is no constructor from an email address, a domain, or a display name. 23.47's failure mode
/// "organizational authority inferred from an email address" is prevented by the absence: the only
/// way to hold an [`OrgIdentity`] is to name an attestation and the authority that issued it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OrgIdentity {
    pub organization: String,
    pub attestation_id: String,
    pub issued_by: String,
}

impl OrgIdentity {
    pub fn attested(
        organization: impl Into<String>,
        attestation_id: impl Into<String>,
        issued_by: impl Into<String>,
    ) -> Self {
        OrgIdentity {
            organization: organization.into(),
            attestation_id: attestation_id.into(),
            issued_by: issued_by.into(),
        }
    }
}

/// The roles 23.47 says an organization may take in a commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgRole {
    Creditor,
    Debtor,
    Approver,
    PolicyAuthority,
}

/// Why an organizational commitment could not survive a session boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum SuccessionRefusal {
    #[error("{organization} has no succession for {obligation}, so the commitment would be orphaned")]
    NoSuccessor {
        organization: String,
        obligation: String,
    },
}

/// An obligation held by an organization rather than a session.
///
/// There is no `discharge_on_session_end`. 23.47 is explicit that these outlive sessions, and the
/// only session-boundary operation offered is [`OrganizationalCommitment::end_session`], which
/// hands the obligation to the next responsible party or refuses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationalCommitment {
    pub identity: OrgIdentity,
    pub role: OrgRole,
    pub obligation: String,
    pub responsible_party: String,
    /// Ordered succession. The first entry not equal to the current party takes over.
    pub succession: Vec<String>,
}

impl OrganizationalCommitment {
    /// Move the commitment to the next responsible party.
    ///
    /// Returns the commitment, never `()`. A session ending cannot make an organizational
    /// obligation go away, so there is no signature in which it does.
    pub fn end_session(mut self) -> Result<Self, SuccessionRefusal> {
        let next = self
            .succession
            .iter()
            .find(|candidate| **candidate != self.responsible_party)
            .cloned()
            .ok_or_else(|| SuccessionRefusal::NoSuccessor {
                organization: self.identity.organization.clone(),
                obligation: self.obligation.clone(),
            })?;
        self.succession.retain(|candidate| *candidate != next);
        self.responsible_party = next;
        Ok(self)
    }
}

/// 23.47's nine mixed-initiative controls a human may exercise.
///
/// 23.47: "The runtime should preserve human edits as typed events rather than pasting them into a
/// system prompt." This crate has no runtime and emits no events, so the list is a vocabulary and
/// the requirement is recorded rather than met.
pub const MIXED_INITIATIVE_CONTROLS: [&str; 9] = [
    "set goals and constraints",
    "challenge an assumption",
    "edit a choreography",
    "take ownership of a continuation",
    "provide missing domain context",
    "narrow or revoke authority",
    "adjudicate a dispute",
    "inspect and replay a branch",
    "stop or dissolve a molecule",
];

/// 23.47's eight evaluation measures, which belong to WeaveBench.
pub const EVALUATION_MEASURES: [&str; 8] = [
    "escalation precision and recall",
    "decision-capsule completeness",
    "human time per resolved high-risk decision",
    "whether critical disagreement is surfaced",
    "whether agents correctly follow human constraints",
    "authority-scope adherence after approval",
    "outcome quality with and without targeted human input",
    "handoff and continuation-resumption success",
];

/// The seven quantities 23.47 asks an attention scheduler to track.
///
/// [`Reviewer`] implements four of them — expertise, fatigue limit, the allowance itself, and
/// duration through the estimate it spends. Interruption cost, queue position and safe reuse of
/// prior human input need a scheduler, which this crate does not have.
pub fn attention_tracking() -> BTreeMap<&'static str, bool> {
    BTreeMap::from([
        ("interruption cost", false),
        ("review duration", true),
        ("queue and availability", false),
        ("required expertise", true),
        ("fatigue and repeated-request limits", true),
        ("urgency and risk", true),
        ("whether prior human input can be reused safely", false),
    ])
}
