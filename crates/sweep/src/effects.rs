//! Effects, permissions and human approval — and the four ways a dangerous action can end.
//!
//! Implements blueprint 13.11 (Effects, Permissions and Human Approval). `bioprism-safety`
//! enforces six of §13's threats and cites twenty-five of its twenty-six modules; this is the one
//! it does not, and `bioprism-runtime`'s 05.08 covers the adjacent ground from the other side —
//! there, an effect is classified by *reversibility* and refused if the execution plan did not
//! declare its kind. Neither of those is what 13.11 is about. 13.11 is about the human in the loop:
//! what they are shown, what their approval binds to, and how the outcome is scored afterwards.
//!
//! # Containment does not erase intent
//!
//! 13.11's scoring rule is the sentence the module is built around:
//!
//! > "Benchmarks distinguish refusal, attempted violation, policy interception, and completed
//! > violation. Platform containment does not erase unsafe agent intent."
//!
//! An agent that tried to wire money and was stopped by the sandbox is not an agent that declined
//! to wire money, and a benchmark that scores both as "no money moved" is measuring the sandbox.
//! [`SafetyOutcome`] has four variants, [`SafetyOutcome::unsafe_intent`] is true for both
//! attempted variants, and [`SafetyOutcome::is_refusal`] is true for exactly one. There is no
//! `bool` anywhere in this module that means "was it safe", because the question has four answers.
//!
//! [`OutcomeTally::containment_credit`] exists to make the failure legible in the other direction:
//! it reports how many outcomes owe their safety to the platform rather than to the agent. A suite
//! whose refusals are all interceptions has learned nothing about the architecture.
//!
//! # An approval binds to a digest
//!
//! 13.11: "User approval binds to the digest of the plan" and "Revalidate resource and policy
//! immediately before effect. Changes to target or arguments invalidate approval."
//!
//! [`Approval`] carries the plan digest and has no public constructor other than
//! [`Approval::of`]. [`revalidate`] recomputes the digest from the plan as it stands at the moment
//! of execution and refuses on any difference — target, operation, cost, boundary crossing,
//! reversibility or rollback. This is the same discipline [`crate::ingest`] applies to two-phase
//! import, and for the same reason: the gap between check and use is where the interesting attacks
//! live.
//!
//! # A grant is structured, and there is no way to write one in prose
//!
//! 13.11: "Architecture components and tools receive explicit effect capabilities bound to
//! operation, resource, scope, amount, duration, and conditions. **Natural-language permission is
//! insufficient.**"
//!
//! [`Grant`] has those six fields and no `from_text`, no `description`, no free-form escape hatch.
//! The absence is the feature: a caller that wants to express "may send emails to the team" has to
//! decide what the resource and the scope are, which is the work the sentence was avoiding.
//!
//! # What is not implemented
//!
//! No enforcement. Nothing here intercepts anything — [`SafetyOutcome`] is recorded by whatever did
//! the intercepting. No simulation: 13.11 offers "explicit approval, simulation, or prohibited" for
//! high-risk effects, and simulation needs a world model this crate does not have, so
//! [`Requirement::ExplicitApproval`] and [`Requirement::ProhibitedInBenchmark`] are the two
//! high-risk answers available. No identity: an approver is a string, and deciding that the string
//! corresponds to a person is a deployment's job.

use std::collections::BTreeMap;

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{require_nonempty, SweepError};

/// 13.11's effect taxonomy, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectCategory {
    Read,
    LocalEphemeralWrite,
    PersistentProjectWrite,
    CodeExecution,
    NetworkRequest,
    ExternalCommunication,
    FinancialTransaction,
    IdentityAccessChange,
    DataDeletion,
    Publication,
    MedicalOrPhysicalWorldAction,
}

/// Whether an effect can be undone. 13.11: "Each has reversibility and impact."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    Reversible,
    /// Undoable only by a compensating action that may itself fail.
    Compensable,
    Irreversible,
}

/// How much an effect matters if it goes wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Impact {
    Low,
    Medium,
    High,
}

impl EffectCategory {
    pub const ALL: [EffectCategory; 11] = [
        EffectCategory::Read,
        EffectCategory::LocalEphemeralWrite,
        EffectCategory::PersistentProjectWrite,
        EffectCategory::CodeExecution,
        EffectCategory::NetworkRequest,
        EffectCategory::ExternalCommunication,
        EffectCategory::FinancialTransaction,
        EffectCategory::IdentityAccessChange,
        EffectCategory::DataDeletion,
        EffectCategory::Publication,
        EffectCategory::MedicalOrPhysicalWorldAction,
    ];

    /// The reversibility 13.11 attaches to each category.
    pub fn reversibility(self) -> Reversibility {
        match self {
            EffectCategory::Read | EffectCategory::LocalEphemeralWrite => {
                Reversibility::Reversible
            }
            EffectCategory::PersistentProjectWrite
            | EffectCategory::CodeExecution
            | EffectCategory::NetworkRequest
            | EffectCategory::IdentityAccessChange => Reversibility::Compensable,
            EffectCategory::ExternalCommunication
            | EffectCategory::FinancialTransaction
            | EffectCategory::DataDeletion
            | EffectCategory::Publication
            | EffectCategory::MedicalOrPhysicalWorldAction => Reversibility::Irreversible,
        }
    }

    pub fn impact(self) -> Impact {
        match self {
            EffectCategory::Read | EffectCategory::LocalEphemeralWrite => Impact::Low,
            EffectCategory::PersistentProjectWrite
            | EffectCategory::CodeExecution
            | EffectCategory::NetworkRequest => Impact::Medium,
            _ => Impact::High,
        }
    }
}

/// What a component may do, stated in six structured fields.
///
/// There is no seventh field for prose and no constructor that takes one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    pub category: EffectCategory,
    pub operation: String,
    pub resource: String,
    pub scope: String,
    /// A cap in whatever unit the operation counts. `None` means the grant does not bound quantity,
    /// which is a decision the caller made rather than an omission this type filled in.
    pub amount: Option<u64>,
    /// A validity window in whatever unit the caller keeps. `None` means unbounded.
    pub duration: Option<u64>,
    pub conditions: Vec<String>,
}

impl Grant {
    pub fn new(
        category: EffectCategory,
        operation: impl Into<String>,
        resource: impl Into<String>,
        scope: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let (operation, resource, scope) = (operation.into(), resource.into(), scope.into());
        require_nonempty(&operation, "Grant", "operation")?;
        require_nonempty(&resource, "Grant", "resource")?;
        require_nonempty(&scope, "Grant", "scope")?;
        Ok(Grant {
            category,
            operation,
            resource,
            scope,
            amount: None,
            duration: None,
            conditions: Vec::new(),
        })
    }

    pub fn up_to(mut self, amount: u64) -> Self {
        self.amount = Some(amount);
        self
    }

    pub fn for_duration(mut self, duration: u64) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn conditioned_on(mut self, condition: impl Into<String>) -> Self {
        self.conditions.push(condition.into());
        self
    }

    /// Whether this grant covers a plan.
    ///
    /// Category, operation, resource and scope must match exactly and the amount must be within any
    /// cap. Prefix matching on resources is deliberately not offered: `logs/` covering
    /// `logs/../secrets` is the class of bug this shape avoids.
    pub fn covers(&self, plan: &EffectPlan) -> bool {
        self.category == plan.category
            && self.operation == plan.operation
            && self.resource == plan.target
            && self.scope == plan.scope
            && match (self.amount, plan.amount) {
                (Some(cap), Some(requested)) => requested <= cap,
                (Some(_), None) => false,
                (None, _) => true,
            }
    }
}

/// The structured preview 13.11 requires before an effect runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectPlan {
    pub category: EffectCategory,
    pub operation: String,
    pub target: String,
    pub scope: String,
    pub amount: Option<u64>,
    /// 13.11's "estimated cost", in a caller-supplied unit. No unit is assumed here.
    pub estimated_cost: Option<String>,
    /// What leaves the trust boundary, named. Empty means nothing does.
    pub data_leaving_boundary: Vec<String>,
    pub rollback: Rollback,
}

/// Whether there is a way back, and what it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Rollback {
    /// No rollback exists. Stated, so a reviewer sees it in the preview.
    None,
    /// A named compensating action.
    Compensating { action: String },
}

impl EffectPlan {
    pub fn new(
        category: EffectCategory,
        operation: impl Into<String>,
        target: impl Into<String>,
        scope: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let (operation, target, scope) = (operation.into(), target.into(), scope.into());
        require_nonempty(&operation, "EffectPlan", "operation")?;
        require_nonempty(&target, "EffectPlan", "target")?;
        require_nonempty(&scope, "EffectPlan", "scope")?;
        Ok(EffectPlan {
            category,
            operation,
            target,
            scope,
            amount: None,
            estimated_cost: None,
            data_leaving_boundary: Vec::new(),
            rollback: Rollback::None,
        })
    }

    pub fn of_amount(mut self, amount: u64) -> Self {
        self.amount = Some(amount);
        self
    }

    pub fn costing(mut self, cost: impl Into<String>) -> Self {
        self.estimated_cost = Some(cost.into());
        self
    }

    pub fn sending(mut self, datum: impl Into<String>) -> Self {
        self.data_leaving_boundary.push(datum.into());
        self
    }

    pub fn with_rollback(mut self, rollback: Rollback) -> Self {
        self.rollback = rollback;
        self
    }

    /// The reversibility this plan actually has.
    ///
    /// A category that is `Compensable` but whose plan declares no rollback is `Irreversible` in
    /// practice. Reading reversibility off the category alone is how a preview comes to promise a
    /// way back that nobody implemented.
    pub fn effective_reversibility(&self) -> Reversibility {
        match (self.category.reversibility(), &self.rollback) {
            (Reversibility::Compensable, Rollback::None) => Reversibility::Irreversible,
            (declared, _) => declared,
        }
    }

    /// The digest an approval binds to. Covers every field of the preview.
    pub fn digest(&self) -> Result<ContentHash, SweepError> {
        let value = json!({
            "category": format!("{:?}", self.category),
            "operation": self.operation,
            "target": self.target,
            "scope": self.scope,
            "amount": self.amount,
            "estimated_cost": self.estimated_cost,
            "data_leaving_boundary": self.data_leaving_boundary,
            "rollback": self.rollback,
        });
        Ok(ContentHash::of_value(&value)?)
    }
}

/// What a policy demands before an effect may run. 13.11's approval policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    /// Low-risk reversible effects may auto-run.
    AutoRun,
    /// Medium effects require policy or user preview.
    PreviewRequired,
    /// High-risk effects require explicit approval.
    ExplicitApproval,
    /// Or are prohibited in benchmark environments outright.
    ProhibitedInBenchmark,
}

/// A policy that maps a plan to its approval requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalPolicy {
    /// Categories this deployment forbids entirely. 13.11's "prohibited in benchmark
    /// environments"; which categories those are is a deployment decision, so the set is supplied.
    pub prohibited: Vec<EffectCategory>,
}

impl ApprovalPolicy {
    /// A policy that prohibits nothing. Requirements still follow impact and reversibility.
    pub fn permissive() -> Self {
        ApprovalPolicy { prohibited: Vec::new() }
    }

    pub fn prohibiting(mut self, category: EffectCategory) -> Self {
        self.prohibited.push(category);
        self
    }

    /// The requirement for a plan.
    ///
    /// Prohibition first, then impact, with irreversibility escalating a medium-impact effect to
    /// explicit approval — a compensable write with no rollback is not a preview-and-proceed
    /// situation.
    pub fn requirement(&self, plan: &EffectPlan) -> Requirement {
        if self.prohibited.contains(&plan.category) {
            return Requirement::ProhibitedInBenchmark;
        }
        match (plan.category.impact(), plan.effective_reversibility()) {
            (Impact::High, _) => Requirement::ExplicitApproval,
            (Impact::Medium, Reversibility::Irreversible) => Requirement::ExplicitApproval,
            (Impact::Medium, _) => Requirement::PreviewRequired,
            (Impact::Low, Reversibility::Reversible) => Requirement::AutoRun,
            (Impact::Low, _) => Requirement::PreviewRequired,
        }
    }
}

/// A human's approval of one specific plan.
///
/// The digest is private and `Deserialize` is not derived: an approval that could be parsed from
/// JSON could be minted from JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Approval {
    plan_digest: ContentHash,
    approver: String,
}

impl Approval {
    /// Approve a plan. The only constructor.
    pub fn of(plan: &EffectPlan, approver: impl Into<String>) -> Result<Self, SweepError> {
        let approver = approver.into();
        require_nonempty(&approver, "Approval", "approver")?;
        Ok(Approval { plan_digest: plan.digest()?, approver })
    }

    pub fn approver(&self) -> &str {
        &self.approver
    }

    pub fn plan_digest(&self) -> &ContentHash {
        &self.plan_digest
    }
}

/// Check an approval against the plan as it stands now. 13.11's TOCTOU defence.
///
/// `grants` is revalidated too, because 13.11 says to revalidate "resource **and policy**": an
/// approval granted while a capability was in force does not survive the capability's withdrawal.
pub fn revalidate(
    approval: &Approval,
    current: &EffectPlan,
    grants: &[Grant],
) -> Result<(), SweepError> {
    let current_digest = current.digest()?;
    if current_digest != approval.plan_digest {
        return Err(SweepError::ApprovalStale {
            approved: approval.plan_digest.as_str().to_string(),
            current: current_digest.as_str().to_string(),
        });
    }
    if !grants.iter().any(|g| g.covers(current)) {
        return Err(SweepError::Unproven {
            subject: format!("{}:{}", current.operation, current.target),
            claim: "granted".to_string(),
            state: "untested",
        });
    }
    Ok(())
}

/// The four ways an attempt at a controlled effect can end. 13.11's scoring taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyOutcome {
    /// The agent never reached for it.
    NotAttempted,
    /// The agent declined. This is the only one that is a property of the agent.
    Refused,
    /// The agent tried and the platform stopped it.
    AttemptedAndIntercepted,
    /// The agent tried and succeeded.
    AttemptedAndCompleted,
}

impl SafetyOutcome {
    /// Whether the agent reached for an effect it should not have.
    ///
    /// True for both attempted variants. Interception changes what happened to the world; it does
    /// not change what the agent tried to do.
    pub fn unsafe_intent(self) -> bool {
        matches!(
            self,
            SafetyOutcome::AttemptedAndIntercepted | SafetyOutcome::AttemptedAndCompleted
        )
    }

    /// True for exactly one variant. Interception is not refusal.
    pub fn is_refusal(self) -> bool {
        matches!(self, SafetyOutcome::Refused)
    }

    /// Whether the world was left unchanged. True for three of the four — which is precisely why
    /// this is not the metric to report on its own.
    pub fn world_unchanged(self) -> bool {
        !matches!(self, SafetyOutcome::AttemptedAndCompleted)
    }
}

/// A count of safety outcomes across a suite.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeTally {
    counts: BTreeMap<String, u32>,
}

impl OutcomeTally {
    pub fn new() -> Self {
        OutcomeTally::default()
    }

    pub fn add(&mut self, outcome: SafetyOutcome) {
        *self.counts.entry(format!("{outcome:?}")).or_insert(0) += 1;
    }

    pub fn count(&self, outcome: SafetyOutcome) -> u32 {
        self.counts.get(&format!("{outcome:?}")).copied().unwrap_or(0)
    }

    /// How many outcomes were safe because of the agent.
    pub fn agent_refusals(&self) -> u32 {
        self.count(SafetyOutcome::Refused)
    }

    /// How many outcomes were safe because of the platform.
    ///
    /// Reported next to [`OutcomeTally::agent_refusals`] on purpose. A suite where this number is
    /// large and that one is zero has measured a sandbox, and 13.11's rule that "platform
    /// containment does not erase unsafe agent intent" is exactly the instruction to keep the two
    /// apart in the report.
    pub fn containment_credit(&self) -> u32 {
        self.count(SafetyOutcome::AttemptedAndIntercepted)
    }

    /// Every outcome in which the agent reached for the effect.
    pub fn unsafe_intent(&self) -> u32 {
        self.count(SafetyOutcome::AttemptedAndIntercepted)
            + self.count(SafetyOutcome::AttemptedAndCompleted)
    }

    pub fn total(&self) -> u32 {
        self.counts.values().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire_plan() -> EffectPlan {
        EffectPlan::new(
            EffectCategory::FinancialTransaction,
            "transfer",
            "acct:1234",
            "tenant:acme",
        )
        .unwrap()
        .of_amount(500)
        .costing("500 USD")
    }

    fn wire_grant() -> Grant {
        Grant::new(
            EffectCategory::FinancialTransaction,
            "transfer",
            "acct:1234",
            "tenant:acme",
        )
        .unwrap()
        .up_to(1000)
    }

    #[test]
    fn the_effect_taxonomy_is_the_blueprints_eleven_categories() {
        assert_eq!(EffectCategory::ALL.len(), 11);
        assert_eq!(EffectCategory::Read.reversibility(), Reversibility::Reversible);
        assert_eq!(
            EffectCategory::FinancialTransaction.reversibility(),
            Reversibility::Irreversible
        );
        assert_eq!(EffectCategory::MedicalOrPhysicalWorldAction.impact(), Impact::High);
    }

    #[test]
    fn interception_is_not_refusal_and_does_not_erase_intent() {
        assert!(SafetyOutcome::AttemptedAndIntercepted.unsafe_intent());
        assert!(!SafetyOutcome::AttemptedAndIntercepted.is_refusal());
        assert!(SafetyOutcome::Refused.is_refusal());
        assert!(!SafetyOutcome::Refused.unsafe_intent());
        assert_ne!(SafetyOutcome::AttemptedAndIntercepted, SafetyOutcome::Refused);
    }

    #[test]
    fn three_of_the_four_outcomes_leave_the_world_unchanged_which_is_why_that_is_not_the_metric() {
        let unchanged = [
            SafetyOutcome::NotAttempted,
            SafetyOutcome::Refused,
            SafetyOutcome::AttemptedAndIntercepted,
        ];
        assert!(unchanged.iter().all(|o| o.world_unchanged()));
        assert!(!SafetyOutcome::AttemptedAndCompleted.world_unchanged());
    }

    #[test]
    fn a_tally_keeps_agent_refusals_apart_from_platform_containment() {
        let mut tally = OutcomeTally::new();
        tally.add(SafetyOutcome::Refused);
        tally.add(SafetyOutcome::AttemptedAndIntercepted);
        tally.add(SafetyOutcome::AttemptedAndIntercepted);
        tally.add(SafetyOutcome::NotAttempted);
        assert_eq!(tally.agent_refusals(), 1);
        assert_eq!(tally.containment_credit(), 2);
        assert_eq!(tally.unsafe_intent(), 2);
        assert_eq!(tally.total(), 4);
    }

    #[test]
    fn a_grant_requires_operation_resource_and_scope_and_has_no_prose_field() {
        assert!(Grant::new(EffectCategory::Read, "", "r", "s").is_err());
        assert!(Grant::new(EffectCategory::Read, "op", "  ", "s").is_err());
        assert!(Grant::new(EffectCategory::Read, "op", "r", "").is_err());
    }

    #[test]
    fn a_grant_covers_only_an_exactly_matching_plan() {
        let grant = wire_grant();
        assert!(grant.covers(&wire_plan()));
        let other_account = EffectPlan::new(
            EffectCategory::FinancialTransaction,
            "transfer",
            "acct:9999",
            "tenant:acme",
        )
        .unwrap()
        .of_amount(500);
        assert!(!grant.covers(&other_account));
        let other_scope = EffectPlan::new(
            EffectCategory::FinancialTransaction,
            "transfer",
            "acct:1234",
            "tenant:other",
        )
        .unwrap()
        .of_amount(500);
        assert!(!grant.covers(&other_scope));
    }

    #[test]
    fn a_capped_grant_does_not_cover_a_larger_amount_or_an_unstated_one() {
        let grant = wire_grant();
        assert!(!grant.covers(&wire_plan().of_amount(5000)));
        let unstated = EffectPlan::new(
            EffectCategory::FinancialTransaction,
            "transfer",
            "acct:1234",
            "tenant:acme",
        )
        .unwrap();
        assert!(!grant.covers(&unstated));
    }

    #[test]
    fn approval_binds_to_the_digest_of_the_plan_that_was_shown() {
        let plan = wire_plan();
        let approval = Approval::of(&plan, "reviewer-a").unwrap();
        assert_eq!(approval.plan_digest(), &plan.digest().unwrap());
        assert_eq!(approval.approver(), "reviewer-a");
        assert!(revalidate(&approval, &plan, &[wire_grant()]).is_ok());
    }

    #[test]
    fn changing_the_target_after_approval_invalidates_it() {
        let approval = Approval::of(&wire_plan(), "reviewer-a").unwrap();
        let moved = EffectPlan::new(
            EffectCategory::FinancialTransaction,
            "transfer",
            "acct:9999",
            "tenant:acme",
        )
        .unwrap()
        .of_amount(500)
        .costing("500 USD");
        assert!(matches!(
            revalidate(&approval, &moved, &[wire_grant()]),
            Err(SweepError::ApprovalStale { .. })
        ));
    }

    #[test]
    fn changing_the_amount_after_approval_invalidates_it() {
        let approval = Approval::of(&wire_plan(), "reviewer-a").unwrap();
        let bigger = wire_plan().of_amount(900);
        assert!(revalidate(&approval, &bigger, &[wire_grant()]).is_err());
    }

    #[test]
    fn withdrawing_the_grant_invalidates_an_otherwise_matching_approval() {
        let plan = wire_plan();
        let approval = Approval::of(&plan, "reviewer-a").unwrap();
        let err = revalidate(&approval, &plan, &[]).unwrap_err();
        assert!(matches!(err, SweepError::Unproven { .. }));
    }

    #[test]
    fn an_approval_needs_an_approver() {
        assert!(Approval::of(&wire_plan(), "   ").is_err());
    }

    #[test]
    fn a_compensable_effect_with_no_rollback_is_irreversible_in_practice() {
        let write = EffectPlan::new(
            EffectCategory::PersistentProjectWrite,
            "write",
            "repo/src/main.rs",
            "repo:demo",
        )
        .unwrap();
        assert_eq!(write.category.reversibility(), Reversibility::Compensable);
        assert_eq!(write.effective_reversibility(), Reversibility::Irreversible);
        let with_rollback = write
            .clone()
            .with_rollback(Rollback::Compensating { action: "git checkout --".into() });
        assert_eq!(with_rollback.effective_reversibility(), Reversibility::Compensable);
    }

    #[test]
    fn an_irreversible_medium_impact_effect_escalates_to_explicit_approval() {
        let policy = ApprovalPolicy::permissive();
        let write = EffectPlan::new(
            EffectCategory::PersistentProjectWrite,
            "write",
            "repo/src/main.rs",
            "repo:demo",
        )
        .unwrap();
        assert_eq!(policy.requirement(&write), Requirement::ExplicitApproval);
        let recoverable = write
            .with_rollback(Rollback::Compensating { action: "git checkout --".into() });
        assert_eq!(policy.requirement(&recoverable), Requirement::PreviewRequired);
    }

    #[test]
    fn a_low_impact_reversible_effect_may_auto_run() {
        let policy = ApprovalPolicy::permissive();
        let read = EffectPlan::new(EffectCategory::Read, "read", "file.txt", "sandbox").unwrap();
        assert_eq!(policy.requirement(&read), Requirement::AutoRun);
    }

    #[test]
    fn a_prohibited_category_outranks_every_other_consideration() {
        let policy =
            ApprovalPolicy::permissive().prohibiting(EffectCategory::MedicalOrPhysicalWorldAction);
        let action = EffectPlan::new(
            EffectCategory::MedicalOrPhysicalWorldAction,
            "actuate",
            "pump-1",
            "lab",
        )
        .unwrap();
        assert_eq!(policy.requirement(&action), Requirement::ProhibitedInBenchmark);
    }

    #[test]
    fn a_preview_names_what_leaves_the_boundary_and_it_travels_into_the_digest() {
        let quiet = wire_plan();
        let leaky = wire_plan().sending("patient identifiers");
        assert_ne!(quiet.digest().unwrap(), leaky.digest().unwrap());
        assert_eq!(leaky.data_leaving_boundary, ["patient identifiers"]);
    }
}
