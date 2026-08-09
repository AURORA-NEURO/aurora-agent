//! Prior evaluation outcomes, and the scalar the router optimises.
//!
//! Blueprint 09.08 says the router should "retrieve relevant capability evidence" and "predict
//! outcome, cost, latency, and risk for candidate architectures". This workspace measures exactly
//! one of those four honestly — outcome, via the deterministic oracle of 43.41 — and one proxy
//! for cost, namely how many facts were exposed. Latency and provider risk are **not** modelled
//! here, and the report does not pretend otherwise.
//!
//! ## Why the utility looks like this
//!
//! `crates/baseline/src/compare.rs` establishes the ordering this crate must not break: facts
//! exposed is a *cost*, and it ranks only among strategies that still satisfy the decision
//! contract. A selection is admissible when it preserves the oracle verdict **and** delivers the
//! full protected closure; a selection that reaches the right verdict from an incomplete closure
//! is the most dangerous category in the panel, not a cheap win.
//!
//! Regret accounting needs a scalar, so that lexicographic ordering is encoded as one:
//!
//! ```text
//! utility = 1 - facts_exposed / total_facts      when admissible
//! utility = -1                                   when not
//! ```
//!
//! Admissible utilities live in `[0, 1]`, so the *worst* admissible architecture — full context,
//! exposing the entire world — scores 0 and still strictly dominates every inadmissible one. That
//! is the point. Any encoding that let a small-but-unsound selection outrank an expensive-but-sound
//! one would reintroduce the failure mode `compare.rs` exists to prevent, and the router would
//! then learn to prefer it.

use crate::architecture::Architecture;
use crate::error::RoutingError;
use crate::fingerprint::Fingerprint;
use bioprism_section::OracleStatus;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Utility assigned to any selection that failed the decision contract.
///
/// Strictly below the worst admissible utility, which is 0. Encoding disqualification as a
/// penalty rather than as a small number is what stops the router trading soundness for cost.
pub const INADMISSIBLE_UTILITY: f64 = -1.0;

/// Utility of one architecture on one task.
pub fn utility(admissible: bool, cost_fraction: f64) -> f64 {
    if admissible {
        1.0 - cost_fraction.clamp(0.0, 1.0)
    } else {
        INADMISSIBLE_UTILITY
    }
}

/// One measured outcome: architecture *a* was run on task *t* and this is what happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub task_id: String,
    pub fingerprint: Fingerprint,
    pub architecture: Architecture,
    /// Same verdict and same witnesses as full context.
    pub verdict_preserving: bool,
    /// Every protected-tagged fact retained, as blueprint 43.13 requires before any relevance step.
    pub closure_complete: bool,
    /// The verdict the deterministic oracle reached from this selection.
    ///
    /// Outcome data, never fingerprint input. It is recorded because a reader auditing a routing
    /// report needs to see *what* the architecture concluded, not only whether the conclusion
    /// survived comparison with full context.
    pub status: OracleStatus,
    pub facts_exposed: usize,
    pub total_facts: usize,
}

impl Observation {
    /// Both conditions, exactly as `bioprism_baseline::StrategyResult::admissible` defines them.
    pub fn admissible(&self) -> bool {
        self.verdict_preserving && self.closure_complete
    }

    pub fn cost_fraction(&self) -> f64 {
        if self.total_facts == 0 {
            0.0
        } else {
            self.facts_exposed as f64 / self.total_facts as f64
        }
    }

    pub fn utility(&self) -> f64 {
        utility(self.admissible(), self.cost_fraction())
    }
}

/// An append-only table of prior outcomes.
///
/// The ledger is deliberately dumb. It holds no model, fits nothing, and cannot be updated in
/// place — blueprint 09.11's online-learning policy (guarded exploration, delayed labels,
/// rollback monitoring) is **not implemented in this crate**. What the ledger does provide is the
/// one guarantee routing honesty depends on: [`EvidenceLedger::excluding_task`], and the leak
/// check that uses it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Observation>", into = "Vec<Observation>")]
pub struct EvidenceLedger {
    observations: Vec<Observation>,
}

impl EvidenceLedger {
    pub fn new(observations: impl IntoIterator<Item = Observation>) -> Result<Self, RoutingError> {
        let observations: Vec<Observation> = observations.into_iter().collect();
        let mut seen: BTreeSet<(&str, String)> = BTreeSet::new();
        for observation in &observations {
            if observation.task_id.is_empty() {
                return Err(RoutingError::EmptyTaskId);
            }
            let key = (
                observation.task_id.as_str(),
                observation.architecture.label(),
            );
            if !seen.insert(key) {
                return Err(RoutingError::DuplicateObservation {
                    task: observation.task_id.clone(),
                    architecture: observation.architecture.label(),
                });
            }
        }
        Ok(EvidenceLedger { observations })
    }

    pub fn observations(&self) -> &[Observation] {
        &self.observations
    }

    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }

    pub fn len(&self) -> usize {
        self.observations.len()
    }

    pub fn task_ids(&self) -> BTreeSet<&str> {
        self.observations
            .iter()
            .map(|o| o.task_id.as_str())
            .collect()
    }

    pub fn contains_task(&self, task_id: &str) -> bool {
        self.observations.iter().any(|o| o.task_id == task_id)
    }

    /// The held-out view. Everything a router is allowed to know about an unseen task.
    pub fn excluding_task(&self, task_id: &str) -> EvidenceLedger {
        self.filtered(|observation| observation.task_id != task_id)
    }

    /// A narrower view for stricter holdouts.
    ///
    /// Excluding only the routed task leaves structurally identical sibling tasks in the
    /// evidence, which is realistic — a deployment has usually seen something similar — but it is
    /// not a test of generalising to unseen *structure*. `crate::lab::Holdout` uses this to build
    /// the stricter view.
    pub fn filtered(&self, keep: impl Fn(&Observation) -> bool) -> EvidenceLedger {
        EvidenceLedger {
            observations: self
                .observations
                .iter()
                .filter(|observation| keep(observation))
                .cloned()
                .collect(),
        }
    }

    pub fn observation(&self, task_id: &str, architecture: &Architecture) -> Option<&Observation> {
        self.observations
            .iter()
            .find(|o| o.task_id == task_id && &o.architecture == architecture)
    }

    /// Observations whose task is structurally within `radius` of `fingerprint`.
    pub fn neighbourhood(&self, fingerprint: &Fingerprint, radius: f64) -> Vec<&Observation> {
        self.observations
            .iter()
            .filter(|o| o.fingerprint.distance(fingerprint) <= radius)
            .collect()
    }

    /// The retrospective best choice for a task: what a selector that already knows every outcome
    /// would have picked. This is the Gate 6 oracle, and it is a *bound*, never a router.
    ///
    /// Ties are broken by architecture label so the bound is reproducible.
    pub fn best_for_task(&self, task_id: &str) -> Option<&Observation> {
        self.observations
            .iter()
            .filter(|o| o.task_id == task_id)
            .max_by(|left, right| {
                left.utility()
                    .partial_cmp(&right.utility())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| right.architecture.label().cmp(&left.architecture.label()))
            })
    }

    /// The architecture with the highest mean cost across observed tasks.
    ///
    /// This is the Gate 6 "largest / most expensive default" comparator, derived from measurement
    /// rather than declared. On any world containing at least one fact it is full context, which
    /// [`crate::lab`] checks rather than assumes.
    pub fn most_expensive_architecture(&self) -> Option<Architecture> {
        let mut by_architecture: Vec<(Architecture, f64, usize)> = Vec::new();
        for observation in &self.observations {
            match by_architecture
                .iter_mut()
                .find(|(architecture, _, _)| architecture == &observation.architecture)
            {
                Some((_, total, count)) => {
                    *total += observation.cost_fraction();
                    *count += 1;
                }
                None => by_architecture.push((
                    observation.architecture.clone(),
                    observation.cost_fraction(),
                    1,
                )),
            }
        }

        by_architecture
            .into_iter()
            .max_by(|left, right| {
                let left_mean = left.1 / left.2 as f64;
                let right_mean = right.1 / right.2 as f64;
                left_mean
                    .partial_cmp(&right_mean)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| right.0.label().cmp(&left.0.label()))
            })
            .map(|(architecture, _, _)| architecture)
    }
}

impl TryFrom<Vec<Observation>> for EvidenceLedger {
    type Error = RoutingError;

    fn try_from(value: Vec<Observation>) -> Result<Self, Self::Error> {
        EvidenceLedger::new(value)
    }
}

impl From<EvidenceLedger> for Vec<Observation> {
    fn from(value: EvidenceLedger) -> Self {
        value.observations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::observation;

    #[test]
    fn an_unsound_selection_scores_below_the_most_expensive_sound_one() {
        let cheap_and_wrong = utility(false, 0.01);
        let expensive_and_right = utility(true, 1.0);
        assert!(
            cheap_and_wrong < expensive_and_right,
            "encoding cost above admissibility would teach the router to drop witnesses"
        );
    }

    #[test]
    fn utility_rewards_compression_only_among_admissible_selections() {
        assert!(utility(true, 0.01) > utility(true, 0.5));
        assert_eq!(utility(false, 0.01), utility(false, 0.5));
    }

    #[test]
    fn a_ledger_rejects_two_observations_of_the_same_task_and_architecture() {
        let error = EvidenceLedger::new([
            observation("t1", Architecture::FiberCompiled, true, 11, 761),
            observation("t1", Architecture::FiberCompiled, true, 12, 761),
        ])
        .unwrap_err();
        assert_eq!(
            error,
            RoutingError::DuplicateObservation {
                task: "t1".to_string(),
                architecture: "fiber".to_string()
            }
        );
    }

    #[test]
    fn excluding_a_task_removes_every_observation_of_that_task() {
        let ledger = EvidenceLedger::new([
            observation("t1", Architecture::FiberCompiled, true, 11, 761),
            observation("t1", Architecture::FullContext, true, 761, 761),
            observation("t2", Architecture::FiberCompiled, true, 11, 761),
        ])
        .unwrap();

        let held_out = ledger.excluding_task("t1");
        assert!(!held_out.contains_task("t1"));
        assert_eq!(held_out.len(), 1);
        assert!(
            ledger.contains_task("t1"),
            "the original ledger is unchanged"
        );
    }

    #[test]
    fn the_oracle_picks_the_cheapest_admissible_architecture_for_the_task() {
        let ledger = EvidenceLedger::new([
            observation("t1", Architecture::FullContext, true, 761, 761),
            observation("t1", Architecture::FiberCompiled, true, 11, 761),
            observation("t1", Architecture::LexicalTopK { k: 11 }, false, 4, 761),
        ])
        .unwrap();

        assert_eq!(
            ledger.best_for_task("t1").unwrap().architecture,
            Architecture::FiberCompiled
        );
    }

    #[test]
    fn the_oracle_never_picks_an_unsound_architecture_however_cheap_it_looks() {
        let ledger = EvidenceLedger::new([
            observation("t1", Architecture::FullContext, true, 761, 761),
            observation("t1", Architecture::LexicalTopK { k: 11 }, false, 1, 761),
        ])
        .unwrap();

        assert_eq!(
            ledger.best_for_task("t1").unwrap().architecture,
            Architecture::FullContext
        );
    }

    #[test]
    fn the_most_expensive_default_is_derived_from_measured_cost() {
        let ledger = EvidenceLedger::new([
            observation("t1", Architecture::FullContext, true, 761, 761),
            observation("t1", Architecture::FiberCompiled, true, 11, 761),
            observation("t2", Architecture::FullContext, true, 40, 40),
            observation("t2", Architecture::FiberCompiled, true, 11, 40),
        ])
        .unwrap();

        assert_eq!(
            ledger.most_expensive_architecture(),
            Some(Architecture::FullContext)
        );
    }

    #[test]
    fn a_ledger_round_trips_through_json_and_revalidates_on_the_way_back() {
        let ledger = EvidenceLedger::new([
            observation("t1", Architecture::FiberCompiled, true, 11, 761),
            observation("t2", Architecture::FiberCompiled, false, 3, 761),
        ])
        .unwrap();
        let text = serde_json::to_string(&ledger).unwrap();
        assert_eq!(
            serde_json::from_str::<EvidenceLedger>(&text).unwrap(),
            ledger
        );

        let duplicated = serde_json::to_string(&vec![
            ledger.observations()[0].clone(),
            ledger.observations()[0].clone(),
        ])
        .unwrap();
        assert!(serde_json::from_str::<EvidenceLedger>(&duplicated).is_err());
    }
}
