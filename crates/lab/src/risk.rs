//! Risk-triggered branching, and the honest accounting of what it cost.
//!
//! Blueprint 09.07: *"Spend additional inference-time computation only when uncertainty,
//! irreversibility, or potential harm justifies it."* Its risk-feature list is nine long, its
//! action list eight, and its evaluation section asks for "success, harmful action rate, decision
//! latency, cost, false escalations, and whether branching helps on states predicted to benefit".
//!
//! Two decisions shape this module.
//!
//! **The trigger is a stated predicate, not a learned threshold.** [`Trigger`] is an algebra over
//! named features with literal bounds, and [`Trigger::stated`] renders any trigger as the sentence
//! a reviewer would have to agree with before the escalation happened. A learned threshold can be
//! right more often and still cannot be reviewed, and 09.07's whole justification is that the extra
//! compute was *justified* — which is a claim about reasons, not about accuracy.
//!
//! **The ledger reports the case where branching cost something and caught nothing.**
//! [`BranchYield::verdict`] checks [`BranchVerdict::PaidAndCaughtNothing`] before every other
//! arm, in the same spirit as `bioprism-routing`'s report checking "the router lost" first. A
//! branch controller that reports only its catches is a branch controller with an unbounded budget.
//!
//! # Three-valued triggers
//!
//! A feature can be unmeasured, and unmeasured is not zero — the workspace's oldest rule.
//! [`RiskFeatures::historical_failure_rate`] is an `Option<f64>` and a predicate over an absent
//! value evaluates to [`TriggerOutcome::Undetermined`], which propagates through `all` and `any`
//! by Kleene's rules. What to do about it is then a stated policy
//! ([`BranchPolicy::on_undetermined`]) rather than an accident of how the comparison was written.
//! The default is [`UndeterminedPolicy::Escalate`], which is fail-closed: a risk nobody measured
//! is not a risk that was measured to be low.
//!
//! # Not implemented, deliberately
//!
//! No execution. This module *plans* branches; forking a suffix, replaying it and comparing the
//! results is `bioprism-runtime`'s fork/replay machinery (blueprint 05.05), and duplicating it
//! here would produce a second execution model to keep in sync. No verifier: [`BranchAction`]
//! names invoking one and this crate cannot run one. No latency measurement and no clock — cost is
//! counted in branches and verifier calls, which are the two quantities a planner controls. No
//! learned risk model, no calibration of risk to outcome, and no automatic threshold tuning; the
//! numbers in a [`Trigger`] are written by a person.

use crate::error::LabError;
use serde::{Deserialize, Serialize};

/// How hard the action is to undo. Ordered: later variants are riskier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    Reversible,
    ReversibleWithCost,
    Irreversible,
}

/// What the action is permitted to touch. Ordered: later variants are riskier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    ReadOnly,
    WriteScoped,
    WriteBroad,
    ExternalEffect,
}

/// What is at stake, as an ordinal band rather than a currency amount.
///
/// A band because the alternative invites a threshold expressed in units this crate has no way to
/// compare across decisions, and a policy that reads "escalate above 5000" is not portable between
/// two callers who disagree about what the 5000 counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueBand {
    Negligible,
    Low,
    Moderate,
    High,
    Severe,
}

impl ValueBand {
    pub fn as_str(self) -> &'static str {
        match self {
            ValueBand::Negligible => "negligible",
            ValueBand::Low => "low",
            ValueBand::Moderate => "moderate",
            ValueBand::High => "high",
            ValueBand::Severe => "severe",
        }
    }
}

/// The state a branch decision is made from. 09.07's risk-feature list, minus what cannot be
/// computed without a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskFeatures {
    pub reversibility: Reversibility,
    pub permission: PermissionLevel,
    pub value_at_stake: ValueBand,
    /// Live hypotheses that no evidence has separated. Comes from
    /// [`crate::hypothesis::SeparationVerdict`], which is why that module is priority one.
    pub unseparated_hypotheses: usize,
    /// Mandatory obligations still undischarged, from `bioprism-obligation`.
    pub unmet_mandatory_obligations: usize,
    /// Historical failure rate for this decision class. `None` means nobody measured it, which is
    /// not the same as zero and is not treated as zero.
    pub historical_failure_rate: Option<f64>,
    pub verifier_available: bool,
}

impl RiskFeatures {
    /// The lowest-risk state, as a base to modify. Not a default in the `Default` sense: calling
    /// this is a claim that the action is reversible, read-only and negligible, and that claim
    /// should be visible at the call site.
    pub fn benign() -> Self {
        RiskFeatures {
            reversibility: Reversibility::Reversible,
            permission: PermissionLevel::ReadOnly,
            value_at_stake: ValueBand::Negligible,
            unseparated_hypotheses: 0,
            unmet_mandatory_obligations: 0,
            historical_failure_rate: Some(0.0),
            verifier_available: true,
        }
    }
}

/// A predicate's three possible answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerOutcome {
    Fired,
    DidNotFire,
    /// A feature the predicate needs was never measured.
    Undetermined,
}

impl TriggerOutcome {
    fn negate(self) -> Self {
        match self {
            TriggerOutcome::Fired => TriggerOutcome::DidNotFire,
            TriggerOutcome::DidNotFire => TriggerOutcome::Fired,
            TriggerOutcome::Undetermined => TriggerOutcome::Undetermined,
        }
    }

    fn of(condition: bool) -> Self {
        if condition {
            TriggerOutcome::Fired
        } else {
            TriggerOutcome::DidNotFire
        }
    }
}

/// An explicitly stated risk predicate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "trigger")]
pub enum Trigger {
    ReversibilityAtLeast {
        level: Reversibility,
    },
    PermissionAtLeast {
        level: PermissionLevel,
    },
    ValueAtStakeAtLeast {
        band: ValueBand,
    },
    UnseparatedHypothesesAtLeast {
        count: usize,
    },
    UnmetMandatoryObligationsAtLeast {
        count: usize,
    },
    /// Fires when the measured rate is at or above `rate`, and is
    /// [`TriggerOutcome::Undetermined`] when no rate was measured.
    HistoricalFailureRateAtLeast {
        rate: f64,
    },
    NoVerifierAvailable,
    All {
        of: Vec<Trigger>,
    },
    Any {
        of: Vec<Trigger>,
    },
    Not {
        of: Box<Trigger>,
    },
}

impl Trigger {
    pub fn evaluate(&self, features: &RiskFeatures) -> TriggerOutcome {
        match self {
            Trigger::ReversibilityAtLeast { level } => {
                TriggerOutcome::of(features.reversibility >= *level)
            }
            Trigger::PermissionAtLeast { level } => {
                TriggerOutcome::of(features.permission >= *level)
            }
            Trigger::ValueAtStakeAtLeast { band } => {
                TriggerOutcome::of(features.value_at_stake >= *band)
            }
            Trigger::UnseparatedHypothesesAtLeast { count } => {
                TriggerOutcome::of(features.unseparated_hypotheses >= *count)
            }
            Trigger::UnmetMandatoryObligationsAtLeast { count } => {
                TriggerOutcome::of(features.unmet_mandatory_obligations >= *count)
            }
            Trigger::HistoricalFailureRateAtLeast { rate } => {
                match features.historical_failure_rate {
                    Some(measured) => TriggerOutcome::of(measured >= *rate),
                    None => TriggerOutcome::Undetermined,
                }
            }
            Trigger::NoVerifierAvailable => TriggerOutcome::of(!features.verifier_available),
            Trigger::All { of } => {
                let mut undetermined = false;
                for trigger in of {
                    match trigger.evaluate(features) {
                        TriggerOutcome::DidNotFire => return TriggerOutcome::DidNotFire,
                        TriggerOutcome::Undetermined => undetermined = true,
                        TriggerOutcome::Fired => {}
                    }
                }
                if undetermined {
                    TriggerOutcome::Undetermined
                } else {
                    TriggerOutcome::Fired
                }
            }
            Trigger::Any { of } => {
                let mut undetermined = false;
                for trigger in of {
                    match trigger.evaluate(features) {
                        TriggerOutcome::Fired => return TriggerOutcome::Fired,
                        TriggerOutcome::Undetermined => undetermined = true,
                        TriggerOutcome::DidNotFire => {}
                    }
                }
                if undetermined {
                    TriggerOutcome::Undetermined
                } else {
                    TriggerOutcome::DidNotFire
                }
            }
            Trigger::Not { of } => of.evaluate(features).negate(),
        }
    }

    /// The predicate as prose, with its literal bounds. What a reviewer signs off on.
    pub fn stated(&self) -> String {
        match self {
            Trigger::ReversibilityAtLeast { level } => {
                format!("the action is at least {level:?} to undo")
            }
            Trigger::PermissionAtLeast { level } => {
                format!("the action needs at least {level:?} permission")
            }
            Trigger::ValueAtStakeAtLeast { band } => {
                format!("the value at stake is at least {}", band.as_str())
            }
            Trigger::UnseparatedHypothesesAtLeast { count } => {
                format!("at least {count} hypotheses remain unseparated")
            }
            Trigger::UnmetMandatoryObligationsAtLeast { count } => {
                format!("at least {count} mandatory obligations are undischarged")
            }
            Trigger::HistoricalFailureRateAtLeast { rate } => {
                format!("the measured historical failure rate is at least {rate}")
            }
            Trigger::NoVerifierAvailable => "no deterministic verifier is available".to_string(),
            Trigger::All { of } => format!(
                "({})",
                of.iter()
                    .map(Trigger::stated)
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
            Trigger::Any { of } => format!(
                "({})",
                of.iter()
                    .map(Trigger::stated)
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            Trigger::Not { of } => format!("not ({})", of.stated()),
        }
    }

    fn is_vacuous(&self) -> bool {
        matches!(self, Trigger::All { of } if of.is_empty())
    }
}

/// 09.07's action set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchAction {
    ProceedSinglePath,
    RetrieveEvidence,
    GenerateAlternatives,
    SimulateToolResult,
    ForkSuffixes,
    InvokeVerifier,
    RequestHumanApproval,
    Abstain,
}

/// The hard ceiling 09.07 requires branch budgets to obey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchCeiling {
    pub max_branches: u32,
    pub max_verifier_calls: u32,
}

/// What one escalation spent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchCost {
    pub branches: u32,
    pub verifier_calls: u32,
}

impl BranchCost {
    pub fn is_free(&self) -> bool {
        self.branches == 0 && self.verifier_calls == 0
    }

    pub fn plus(self, other: BranchCost) -> BranchCost {
        BranchCost {
            branches: self.branches + other.branches,
            verifier_calls: self.verifier_calls + other.verifier_calls,
        }
    }
}

/// One stated rule: if this predicate holds, spend this much on this action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchRule {
    pub id: String,
    pub trigger: Trigger,
    pub action: BranchAction,
    pub cost: BranchCost,
}

impl BranchRule {
    pub fn new(id: impl Into<String>, trigger: Trigger, action: BranchAction) -> Self {
        BranchRule {
            id: id.into(),
            trigger,
            action,
            cost: BranchCost::default(),
        }
    }

    pub fn spending(mut self, branches: u32, verifier_calls: u32) -> Self {
        self.cost = BranchCost {
            branches,
            verifier_calls,
        };
        self
    }
}

/// What to do when a trigger cannot be evaluated because a feature was never measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UndeterminedPolicy {
    /// Treat as fired. Fail-closed, and the default: an unmeasured risk is not a low risk.
    Escalate,
    /// Treat as not fired. Legitimate for a cheap reversible decision class, and a choice that has
    /// to be written down to be made.
    Proceed,
}

/// The plan for one decision, with the reason attached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchPlan {
    /// The rule that fired, or `None` when nothing did.
    pub rule: Option<String>,
    /// The predicate as prose. Present whenever a rule fired, so a plan always carries its reason.
    pub because: Option<String>,
    pub action: BranchAction,
    pub cost: BranchCost,
    /// Whether the rule fired only because [`UndeterminedPolicy::Escalate`] resolved an
    /// unmeasured feature. Reported separately: escalating on ignorance is a different fact from
    /// escalating on measured risk, and the remedy is to measure.
    pub on_undetermined: bool,
}

/// The declared branch controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchPolicy {
    pub ceiling: BranchCeiling,
    pub on_undetermined: UndeterminedPolicy,
    rules: Vec<BranchRule>,
}

impl BranchPolicy {
    /// Builds a policy, refusing a vacuous trigger and any rule over the hard ceiling.
    ///
    /// A rule whose trigger is vacuously true is an unconditional escalation, which is not
    /// risk-triggered branching however it is labelled.
    pub fn new(
        ceiling: BranchCeiling,
        on_undetermined: UndeterminedPolicy,
        rules: Vec<BranchRule>,
    ) -> Result<Self, LabError> {
        for rule in &rules {
            if rule.trigger.is_vacuous() {
                return Err(LabError::UntriggeredRule(rule.id.clone()));
            }
            if rule.cost.branches > ceiling.max_branches {
                return Err(LabError::BranchCeilingExceeded {
                    requested: rule.cost.branches,
                    ceiling: ceiling.max_branches,
                });
            }
            if rule.cost.verifier_calls > ceiling.max_verifier_calls {
                return Err(LabError::VerifierCeilingExceeded {
                    requested: rule.cost.verifier_calls,
                    ceiling: ceiling.max_verifier_calls,
                });
            }
        }
        Ok(BranchPolicy {
            ceiling,
            on_undetermined,
            rules,
        })
    }

    pub fn rules(&self) -> &[BranchRule] {
        &self.rules
    }

    /// Plans one decision. The first rule whose trigger fires wins; order is the policy's.
    ///
    /// When nothing fires, the plan is [`BranchAction::ProceedSinglePath`] at zero cost and with
    /// no rule named — which is the correct shape, because "no rule required extra compute" is not
    /// the same as "a rule decided to spend nothing".
    pub fn plan(&self, features: &RiskFeatures) -> BranchPlan {
        for rule in &self.rules {
            let outcome = rule.trigger.evaluate(features);
            let fires = match outcome {
                TriggerOutcome::Fired => true,
                TriggerOutcome::DidNotFire => false,
                TriggerOutcome::Undetermined => {
                    self.on_undetermined == UndeterminedPolicy::Escalate
                }
            };
            if fires {
                return BranchPlan {
                    rule: Some(rule.id.clone()),
                    because: Some(rule.trigger.stated()),
                    action: rule.action,
                    cost: rule.cost,
                    on_undetermined: outcome == TriggerOutcome::Undetermined,
                };
            }
        }
        BranchPlan {
            rule: None,
            because: None,
            action: BranchAction::ProceedSinglePath,
            cost: BranchCost::default(),
            on_undetermined: false,
        }
    }
}

/// Something the extra verification found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catch {
    pub what: String,
    /// What would have happened on the single path. The counterfactual is the claim; without it
    /// "the verifier fired" is not evidence that the branch was worth its cost.
    pub would_have_been: String,
}

/// One decision, its plan, and what happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchOutcome {
    pub decision: String,
    pub plan: BranchPlan,
    /// What the extra spend caught, if anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caught: Option<Catch>,
    /// A harm that got through anyway, recorded whether or not a branch fired. Without this the
    /// ledger can only ever report false positives, and 09.07 asks for both directions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escaped: Option<String>,
}

impl BranchOutcome {
    pub fn new(decision: impl Into<String>, plan: BranchPlan) -> Self {
        BranchOutcome {
            decision: decision.into(),
            plan,
            caught: None,
            escaped: None,
        }
    }

    pub fn catching(mut self, what: impl Into<String>, would_have_been: impl Into<String>) -> Self {
        self.caught = Some(Catch {
            what: what.into(),
            would_have_been: would_have_been.into(),
        });
        self
    }

    pub fn with_escape(mut self, escaped: impl Into<String>) -> Self {
        self.escaped = Some(escaped.into());
        self
    }

    pub fn escalated(&self) -> bool {
        self.plan.rule.is_some() && !self.plan.cost.is_free()
    }
}

/// The verdict a report leads with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum BranchVerdict {
    /// No decision triggered a rule. The controller spent nothing and proved nothing.
    NothingTriggered,
    /// Escalations happened and caught nothing. Checked before every other positive arm.
    PaidAndCaughtNothing {
        spent: BranchCost,
        escalations: usize,
    },
    /// Escalations caught something, and some spent nothing useful. Both numbers, always.
    Mixed {
        spent: BranchCost,
        catches: usize,
        wasted_escalations: usize,
    },
    /// Every escalation caught something.
    EveryEscalationCaughtSomething { spent: BranchCost, catches: usize },
}

/// What the branch controller cost and what it caught.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchYield {
    pub decisions: usize,
    pub escalations: usize,
    /// Escalations that happened only because an unmeasured feature was resolved fail-closed.
    pub escalations_on_undetermined: usize,
    pub spent: BranchCost,
    pub catches: usize,
    /// Escalations that spent budget and caught nothing. 09.07's "false escalations".
    pub wasted_escalations: usize,
    /// Harms that got through despite an escalation.
    pub escaped_after_escalation: usize,
    /// Harms that got through where no rule fired. The trigger set's false negatives.
    pub escaped_without_escalation: usize,
    /// Branches per catch. `None` when nothing was caught — not zero, and not infinity.
    pub branches_per_catch: Option<f64>,
}

impl BranchYield {
    /// States the result, checking the unflattering case first.
    pub fn verdict(&self) -> BranchVerdict {
        if self.escalations == 0 {
            return BranchVerdict::NothingTriggered;
        }
        if self.catches == 0 {
            return BranchVerdict::PaidAndCaughtNothing {
                spent: self.spent,
                escalations: self.escalations,
            };
        }
        if self.wasted_escalations > 0 {
            return BranchVerdict::Mixed {
                spent: self.spent,
                catches: self.catches,
                wasted_escalations: self.wasted_escalations,
            };
        }
        BranchVerdict::EveryEscalationCaughtSomething {
            spent: self.spent,
            catches: self.catches,
        }
    }
}

/// The record of every decision the controller saw.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BranchLedger {
    outcomes: Vec<BranchOutcome>,
}

impl BranchLedger {
    pub fn new() -> Self {
        BranchLedger::default()
    }

    /// Records a decision. Decisions where nothing triggered must be recorded too — they are the
    /// denominator, and a ledger of escalations alone cannot say whether the trigger set is tight.
    pub fn record(&mut self, outcome: BranchOutcome) {
        self.outcomes.push(outcome);
    }

    pub fn outcomes(&self) -> &[BranchOutcome] {
        &self.outcomes
    }

    pub fn report(&self) -> BranchYield {
        let mut yielded = BranchYield {
            decisions: self.outcomes.len(),
            escalations: 0,
            escalations_on_undetermined: 0,
            spent: BranchCost::default(),
            catches: 0,
            wasted_escalations: 0,
            escaped_after_escalation: 0,
            escaped_without_escalation: 0,
            branches_per_catch: None,
        };
        for outcome in &self.outcomes {
            if outcome.escalated() {
                yielded.escalations += 1;
                yielded.spent = yielded.spent.plus(outcome.plan.cost);
                if outcome.plan.on_undetermined {
                    yielded.escalations_on_undetermined += 1;
                }
                if outcome.caught.is_some() {
                    yielded.catches += 1;
                } else {
                    yielded.wasted_escalations += 1;
                }
                if outcome.escaped.is_some() {
                    yielded.escaped_after_escalation += 1;
                }
            } else if outcome.escaped.is_some() {
                yielded.escaped_without_escalation += 1;
            }
        }
        if yielded.catches > 0 {
            yielded.branches_per_catch =
                Some(f64::from(yielded.spent.branches) / yielded.catches as f64);
        }
        yielded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ceiling() -> BranchCeiling {
        BranchCeiling {
            max_branches: 4,
            max_verifier_calls: 2,
        }
    }

    fn irreversible_rule() -> BranchRule {
        BranchRule::new(
            "irreversible-and-contested",
            Trigger::All {
                of: vec![
                    Trigger::ReversibilityAtLeast {
                        level: Reversibility::Irreversible,
                    },
                    Trigger::UnseparatedHypothesesAtLeast { count: 2 },
                ],
            },
            BranchAction::ForkSuffixes,
        )
        .spending(3, 1)
    }

    fn policy() -> BranchPolicy {
        BranchPolicy::new(
            ceiling(),
            UndeterminedPolicy::Escalate,
            vec![irreversible_rule()],
        )
        .unwrap()
    }

    #[test]
    fn a_rule_with_a_vacuous_trigger_is_not_risk_triggered_branching() {
        assert_eq!(
            BranchPolicy::new(
                ceiling(),
                UndeterminedPolicy::Escalate,
                vec![BranchRule::new(
                    "always",
                    Trigger::All { of: Vec::new() },
                    BranchAction::ForkSuffixes,
                )
                .spending(2, 0)],
            ),
            Err(LabError::UntriggeredRule("always".to_string()))
        );
    }

    #[test]
    fn a_rule_over_the_hard_ceiling_is_refused_at_construction_not_clamped() {
        assert_eq!(
            BranchPolicy::new(
                ceiling(),
                UndeterminedPolicy::Escalate,
                vec![BranchRule::new(
                    "wide",
                    Trigger::NoVerifierAvailable,
                    BranchAction::ForkSuffixes
                )
                .spending(9, 0)],
            ),
            Err(LabError::BranchCeilingExceeded {
                requested: 9,
                ceiling: 4
            })
        );
    }

    #[test]
    fn a_plan_that_escalates_always_carries_the_predicate_that_justified_it() {
        let mut features = RiskFeatures::benign();
        features.reversibility = Reversibility::Irreversible;
        features.unseparated_hypotheses = 3;
        let plan = policy().plan(&features);
        assert_eq!(plan.action, BranchAction::ForkSuffixes);
        assert_eq!(
            plan.because.as_deref(),
            Some("(the action is at least Irreversible to undo and at least 2 hypotheses remain unseparated)")
        );
    }

    #[test]
    fn a_low_risk_decision_names_no_rule_rather_than_a_rule_that_spent_nothing() {
        let plan = policy().plan(&RiskFeatures::benign());
        assert_eq!(plan.rule, None);
        assert_eq!(plan.action, BranchAction::ProceedSinglePath);
        assert!(plan.cost.is_free());
    }

    #[test]
    fn an_unmeasured_failure_rate_is_undetermined_rather_than_zero() {
        let mut features = RiskFeatures::benign();
        features.historical_failure_rate = None;
        assert_eq!(
            Trigger::HistoricalFailureRateAtLeast { rate: 0.1 }.evaluate(&features),
            TriggerOutcome::Undetermined
        );
    }

    #[test]
    fn an_undetermined_trigger_escalates_by_default_and_says_that_is_why() {
        let mut features = RiskFeatures::benign();
        features.historical_failure_rate = None;
        let policy = BranchPolicy::new(
            ceiling(),
            UndeterminedPolicy::Escalate,
            vec![BranchRule::new(
                "risky-class",
                Trigger::HistoricalFailureRateAtLeast { rate: 0.1 },
                BranchAction::InvokeVerifier,
            )
            .spending(0, 1)],
        )
        .unwrap();
        let plan = policy.plan(&features);
        assert_eq!(plan.action, BranchAction::InvokeVerifier);
        assert!(plan.on_undetermined);
    }

    #[test]
    fn a_policy_that_proceeds_on_undetermined_has_written_that_choice_down() {
        let mut features = RiskFeatures::benign();
        features.historical_failure_rate = None;
        let policy = BranchPolicy::new(
            ceiling(),
            UndeterminedPolicy::Proceed,
            vec![BranchRule::new(
                "risky-class",
                Trigger::HistoricalFailureRateAtLeast { rate: 0.1 },
                BranchAction::InvokeVerifier,
            )
            .spending(0, 1)],
        )
        .unwrap();
        assert_eq!(
            policy.plan(&features).action,
            BranchAction::ProceedSinglePath
        );
    }

    #[test]
    fn an_undetermined_conjunct_does_not_rescue_a_conjunction_that_already_failed() {
        let mut features = RiskFeatures::benign();
        features.historical_failure_rate = None;
        let trigger = Trigger::All {
            of: vec![
                Trigger::ReversibilityAtLeast {
                    level: Reversibility::Irreversible,
                },
                Trigger::HistoricalFailureRateAtLeast { rate: 0.1 },
            ],
        };
        assert_eq!(trigger.evaluate(&features), TriggerOutcome::DidNotFire);
    }

    #[test]
    fn branching_that_cost_something_and_caught_nothing_is_reported_as_exactly_that() {
        let mut ledger = BranchLedger::new();
        let mut features = RiskFeatures::benign();
        features.reversibility = Reversibility::Irreversible;
        features.unseparated_hypotheses = 2;
        let plan = policy().plan(&features);
        ledger.record(BranchOutcome::new("d1", plan.clone()));
        ledger.record(BranchOutcome::new("d2", plan));
        let report = ledger.report();
        assert_eq!(report.escalations, 2);
        assert_eq!(report.catches, 0);
        assert_eq!(report.branches_per_catch, None);
        assert_eq!(
            report.verdict(),
            BranchVerdict::PaidAndCaughtNothing {
                spent: BranchCost {
                    branches: 6,
                    verifier_calls: 2
                },
                escalations: 2,
            }
        );
    }

    #[test]
    fn a_mixed_ledger_reports_the_wasted_escalations_alongside_the_catches() {
        let mut ledger = BranchLedger::new();
        let mut features = RiskFeatures::benign();
        features.reversibility = Reversibility::Irreversible;
        features.unseparated_hypotheses = 2;
        let plan = policy().plan(&features);
        ledger.record(
            BranchOutcome::new("d1", plan.clone())
                .catching("wrote to the wrong scope", "an irreversible write to prod"),
        );
        ledger.record(BranchOutcome::new("d2", plan));
        ledger.record(BranchOutcome::new(
            "d3",
            policy().plan(&RiskFeatures::benign()),
        ));
        let report = ledger.report();
        assert_eq!(report.decisions, 3);
        assert_eq!(
            report.verdict(),
            BranchVerdict::Mixed {
                spent: BranchCost {
                    branches: 6,
                    verifier_calls: 2
                },
                catches: 1,
                wasted_escalations: 1,
            }
        );
        assert_eq!(report.branches_per_catch, Some(6.0));
    }

    #[test]
    fn a_harm_that_got_through_where_no_rule_fired_is_counted_as_a_trigger_false_negative() {
        let mut ledger = BranchLedger::new();
        ledger.record(
            BranchOutcome::new("d1", policy().plan(&RiskFeatures::benign()))
                .with_escape("deleted a row nobody could restore"),
        );
        let report = ledger.report();
        assert_eq!(report.escaped_without_escalation, 1);
        assert_eq!(report.verdict(), BranchVerdict::NothingTriggered);
    }

    #[test]
    fn an_empty_ledger_reports_nothing_triggered_rather_than_a_perfect_record() {
        assert_eq!(
            BranchLedger::new().report().verdict(),
            BranchVerdict::NothingTriggered
        );
    }
}
