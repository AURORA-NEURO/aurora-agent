//! The trial ledger: what was actually run, and what it is allowed to count as.
//!
//! Blueprint 08.07 is blunt about the boundary this type defends: "The planner does not count
//! dispatch as evidence." A trial enters the ledger only once it has an outcome, and the ledger
//! is the single place that decides whether that outcome is evidence about the capability.
//!
//! Two rules do most of the work.
//!
//! **Abstention is not failure.** An architecture that declines to answer has told you something
//! about itself and nothing about whether it can do the task. Counting abstentions as failures is
//! the cheapest way to manufacture a capability difference between a cautious system and a
//! reckless one. They are recorded, they consume budget, and they are reported — but they never
//! enter the Bernoulli likelihood and never satisfy a coverage floor.
//!
//! **One scored trial per instance.** The clustered model in [`crate::cluster`] has exactly one
//! level, the parent. Two trials of the same instance are a second, nested level of dependence
//! that it does not represent, and admitting them would let a caller inflate the effective sample
//! size by re-running the same instance. The ledger refuses the second one with a typed error
//! rather than quietly averaging it away.

use crate::cluster::{Cluster, ClusterSummary};
use crate::error::AdaptiveError;
use crate::id::{CapabilityId, InstanceId, ParentId};
use bioprism_section::OracleStatus;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The scored result of one execution trial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
    /// The architecture declined to answer, or the evaluator could not decide.
    ///
    /// Blueprint 43.15 requires abstention to be representable ("No feasible action yields an
    /// abstention certificate") and 43.28 requires it to be reportable. It is neither a success
    /// nor a failure and is excluded from the posterior.
    Abstained,
}

impl Outcome {
    /// The outcome of comparing an architecture's verdict against the reference oracle's.
    ///
    /// A trial passes when the two agree. An architecture that returns
    /// [`OracleStatus::Underdetermined`] has abstained; the *reference* being underdetermined is
    /// a defect in the instance rather than in the architecture, and is likewise not scored.
    pub fn from_oracle_agreement(reference: OracleStatus, observed: OracleStatus) -> Self {
        if observed == OracleStatus::Underdetermined || reference == OracleStatus::Underdetermined {
            Outcome::Abstained
        } else if observed == reference {
            Outcome::Pass
        } else {
            Outcome::Fail
        }
    }

    pub fn is_scored(self) -> bool {
        !matches!(self, Outcome::Abstained)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Fail => "fail",
            Outcome::Abstained => "abstained",
        }
    }
}

/// One execution trial and its scored result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trial {
    pub capability: CapabilityId,
    pub instance: InstanceId,
    /// The audited parent world this instance descends from. The clustering key.
    pub parent: ParentId,
    pub outcome: Outcome,
    /// What running it cost, in whatever unit the caller's budget is denominated in.
    pub cost: f64,
}

impl Trial {
    pub fn new(
        capability: CapabilityId,
        instance: InstanceId,
        parent: ParentId,
        outcome: Outcome,
        cost: f64,
    ) -> Result<Self, AdaptiveError> {
        if !cost.is_finite() || cost <= 0.0 {
            return Err(AdaptiveError::InvalidCost {
                instance: instance.to_string(),
                cost,
            });
        }
        Ok(Trial {
            capability,
            instance,
            parent,
            outcome,
            cost,
        })
    }
}

/// Every trial the panel has observed, in the order it observed them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrialLedger {
    trials: Vec<Trial>,
    /// Instances that already carry scored evidence, by capability.
    ///
    /// Kept as an index rather than derived on demand because selection asks the question once
    /// per candidate per pick, and a linear scan there turns a panel run into a quadratic one.
    scored: BTreeMap<CapabilityId, BTreeSet<InstanceId>>,
}

impl TrialLedger {
    pub fn new() -> Self {
        TrialLedger::default()
    }

    /// Appends a trial, refusing a second scored trial on an instance.
    pub fn record(&mut self, trial: Trial) -> Result<(), AdaptiveError> {
        if trial.outcome.is_scored()
            && !self
                .scored
                .entry(trial.capability.clone())
                .or_default()
                .insert(trial.instance.clone())
        {
            return Err(AdaptiveError::DuplicateTrial {
                capability: trial.capability.to_string(),
                instance: trial.instance.to_string(),
            });
        }
        self.trials.push(trial);
        Ok(())
    }

    pub fn trials(&self) -> &[Trial] {
        &self.trials
    }

    pub fn len(&self) -> usize {
        self.trials.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trials.is_empty()
    }

    /// Every capability with at least one recorded trial, in identifier order.
    pub fn capabilities(&self) -> Vec<CapabilityId> {
        self.trials
            .iter()
            .map(|t| t.capability.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Scored trials for a capability, grouped by parent.
    pub fn summary(&self, capability: &CapabilityId) -> ClusterSummary {
        let mut by_parent: BTreeMap<ParentId, (usize, usize)> = BTreeMap::new();
        for trial in self
            .trials
            .iter()
            .filter(|t| &t.capability == capability && t.outcome.is_scored())
        {
            let entry = by_parent.entry(trial.parent.clone()).or_insert((0, 0));
            entry.0 += 1;
            if trial.outcome == Outcome::Pass {
                entry.1 += 1;
            }
        }
        ClusterSummary::new(
            by_parent
                .into_iter()
                .map(|(parent, (trials, successes))| Cluster {
                    parent,
                    trials,
                    successes,
                })
                .collect(),
        )
    }

    /// Scored trials already spent on one parent within one capability.
    ///
    /// The quantity the acquisition score discounts by; see
    /// [`crate::cluster::marginal_independent_weight`].
    pub fn parent_trials(&self, capability: &CapabilityId, parent: &ParentId) -> usize {
        self.trials
            .iter()
            .filter(|t| &t.capability == capability && &t.parent == parent && t.outcome.is_scored())
            .count()
    }

    /// Whether this instance already contributed scored evidence to this capability.
    ///
    /// Selection uses it to avoid proposing an instance the ledger would then refuse.
    pub fn has_scored(&self, capability: &CapabilityId, instance: &InstanceId) -> bool {
        self.scored
            .get(capability)
            .is_some_and(|instances| instances.contains(instance))
    }

    pub fn abstentions(&self, capability: &CapabilityId) -> usize {
        self.trials
            .iter()
            .filter(|t| &t.capability == capability && t.outcome == Outcome::Abstained)
            .count()
    }

    /// Cost of every trial recorded, abstentions included: they were still run.
    pub fn total_cost(&self) -> f64 {
        self.trials.iter().fold(0.0, |total, t| total + t.cost)
    }

    pub fn cost_of(&self, capability: &CapabilityId) -> f64 {
        self.trials
            .iter()
            .filter(|t| &t.capability == capability)
            .fold(0.0, |total, t| total + t.cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial(capability: &str, instance: &str, parent: &str, outcome: Outcome) -> Trial {
        Trial::new(
            CapabilityId::parse(capability).unwrap(),
            InstanceId::parse(instance).unwrap(),
            ParentId::parse(parent).unwrap(),
            outcome,
            1.0,
        )
        .unwrap()
    }

    #[test]
    fn a_second_scored_trial_on_one_instance_is_refused() {
        let mut ledger = TrialLedger::new();
        ledger
            .record(trial("cap", "inst-1", "p1", Outcome::Pass))
            .unwrap();
        let repeat = ledger.record(trial("cap", "inst-1", "p1", Outcome::Fail));
        assert!(matches!(repeat, Err(AdaptiveError::DuplicateTrial { .. })));
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn an_abstention_does_not_block_a_later_scored_trial_on_the_same_instance() {
        let mut ledger = TrialLedger::new();
        ledger
            .record(trial("cap", "inst-1", "p1", Outcome::Abstained))
            .unwrap();
        ledger
            .record(trial("cap", "inst-1", "p1", Outcome::Pass))
            .unwrap();
        assert_eq!(ledger.len(), 2);
        assert_eq!(ledger.summary(&CapabilityId::parse("cap").unwrap()).trials(), 1);
    }

    #[test]
    fn abstentions_are_excluded_from_the_posterior_but_still_counted_and_costed() {
        let mut ledger = TrialLedger::new();
        let cap = CapabilityId::parse("cap").unwrap();
        ledger
            .record(trial("cap", "i1", "p1", Outcome::Pass))
            .unwrap();
        ledger
            .record(trial("cap", "i2", "p1", Outcome::Abstained))
            .unwrap();
        ledger
            .record(trial("cap", "i3", "p2", Outcome::Fail))
            .unwrap();
        let summary = ledger.summary(&cap);
        assert_eq!(summary.trials(), 2);
        assert_eq!(summary.successes(), 1);
        assert_eq!(ledger.abstentions(&cap), 1);
        assert_eq!(ledger.len(), 3);
        assert_eq!(ledger.total_cost(), 3.0);
    }

    #[test]
    fn the_summary_groups_by_parent_and_not_by_instance() {
        let mut ledger = TrialLedger::new();
        for i in 0..6 {
            let parent = if i < 4 { "p1" } else { "p2" };
            ledger
                .record(trial("cap", &format!("i{i}"), parent, Outcome::Pass))
                .unwrap();
        }
        let summary = ledger.summary(&CapabilityId::parse("cap").unwrap());
        assert_eq!(summary.parents(), 2);
        assert_eq!(summary.trials(), 6);
        assert_eq!(summary.clusters[0].trials, 4);
    }

    #[test]
    fn an_oracle_disagreement_is_a_failure_and_an_architecture_abstention_is_not() {
        assert_eq!(
            Outcome::from_oracle_agreement(OracleStatus::Invalid, OracleStatus::Invalid),
            Outcome::Pass
        );
        assert_eq!(
            Outcome::from_oracle_agreement(OracleStatus::Invalid, OracleStatus::Valid),
            Outcome::Fail
        );
        assert_eq!(
            Outcome::from_oracle_agreement(OracleStatus::Invalid, OracleStatus::Underdetermined),
            Outcome::Abstained
        );
        assert_eq!(
            Outcome::from_oracle_agreement(OracleStatus::Underdetermined, OracleStatus::Valid),
            Outcome::Abstained
        );
    }

    #[test]
    fn the_text_form_of_an_outcome_cannot_drift_from_its_serialized_form() {
        for outcome in [Outcome::Pass, Outcome::Fail, Outcome::Abstained] {
            assert_eq!(
                serde_json::to_string(&outcome).unwrap(),
                format!("\"{}\"", outcome.as_str())
            );
        }
    }

    #[test]
    fn a_trial_with_a_non_positive_cost_is_rejected() {
        let bad = Trial::new(
            CapabilityId::parse("c").unwrap(),
            InstanceId::parse("i").unwrap(),
            ParentId::parse("p").unwrap(),
            Outcome::Pass,
            0.0,
        );
        assert!(matches!(bad, Err(AdaptiveError::InvalidCost { .. })));
    }
}
