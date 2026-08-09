//! Coverage floors, and the refusal to report below them.
//!
//! Blueprint 08.04 exists because efficiency and coverage pull in opposite directions: "Ensure
//! efficiency does not erase rare, difficult, or safety-critical evaluation strata." An
//! information-gain policy left to itself will abandon a capability the moment its posterior
//! stops moving, and a capability the architecture is uniformly terrible at stops moving *fast*.
//! The panel then reports a confident estimate of the capability it found easy and nothing at all
//! about the one that matters.
//!
//! The floors here are hard constraints, not penalties in the objective. Two of them are
//! deliberately more than "count the trials":
//!
//! * A parent counts toward `min_parents_per_capability` only once it has reached
//!   `min_trials_per_parent`. Otherwise a floor of "ten parents" is satisfiable by nine parents
//!   with one trial each plus one parent with ninety-one, which is the clustered configuration
//!   the floor was written to prevent.
//! * `sentinels` are the fixed longitudinal panel of 08.04 — named parents that must be run
//!   whatever the acquisition score says, so that drift in the scheduler or the registry is
//!   detectable at all.
//!
//! When a floor is unmet the estimate is **withheld**, not annotated. See
//! [`crate::error::AdaptiveError::CoverageFloorNotMet`].

use crate::error::AdaptiveError;
use crate::id::{CapabilityId, ParentId};
use crate::ledger::TrialLedger;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One unmet constraint, named concretely enough to act on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shortfall {
    /// Too few scored trials for the capability overall.
    Trials { have: usize, need: usize },
    /// Too few parents that individually reached `min_trials_per_parent`.
    QualifyingParents { have: usize, need: usize },
    /// A mandatory sentinel parent was under-run or never run.
    Sentinel {
        parent: ParentId,
        have: usize,
        need: usize,
    },
    /// One parent supplies too large a share of the capability's evidence.
    ParentDominance {
        parent: ParentId,
        share: f64,
        cap: f64,
    },
}

impl fmt::Display for Shortfall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shortfall::Trials { have, need } => write!(f, "{have}/{need} scored trials"),
            Shortfall::QualifyingParents { have, need } => {
                write!(f, "{have}/{need} qualifying parents")
            }
            Shortfall::Sentinel { parent, have, need } => {
                write!(f, "sentinel {parent} at {have}/{need} trials")
            }
            Shortfall::ParentDominance { parent, share, cap } => {
                write!(f, "parent {parent} supplies {share:.2} of trials (cap {cap:.2})")
            }
        }
    }
}

/// The hard constraints an adaptive panel may not trade away.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoveragePolicy {
    pub min_trials_per_capability: usize,
    pub min_parents_per_capability: usize,
    /// Trials a parent needs before it counts toward `min_parents_per_capability`, and the floor
    /// each sentinel parent must reach.
    pub min_trials_per_parent: usize,
    /// Optional cap on any single parent's share of a capability's scored trials.
    ///
    /// Off by default. The design effect already prices parent dominance into the interval, so
    /// this is a separate *policy* choice — "do not let one parent speak for this capability at
    /// all" — rather than a statistical necessity, and defaults that silently reshape a
    /// scheduler's behaviour are worse than defaults that do nothing.
    pub max_parent_share: Option<f64>,
    /// Mandatory parents per capability: the fixed longitudinal panel of 08.04.
    pub sentinels: BTreeMap<CapabilityId, BTreeSet<ParentId>>,
}

impl Default for CoveragePolicy {
    fn default() -> Self {
        CoveragePolicy {
            min_trials_per_capability: 30,
            min_parents_per_capability: 5,
            min_trials_per_parent: 2,
            max_parent_share: None,
            sentinels: BTreeMap::new(),
        }
    }
}

impl CoveragePolicy {
    pub fn sentinels_for(&self, capability: &CapabilityId) -> &BTreeSet<ParentId> {
        static EMPTY: std::sync::OnceLock<BTreeSet<ParentId>> = std::sync::OnceLock::new();
        self.sentinels
            .get(capability)
            .unwrap_or_else(|| EMPTY.get_or_init(BTreeSet::new))
    }

    /// Evaluates the floors for one capability against the ledger.
    pub fn status(&self, ledger: &TrialLedger, capability: &CapabilityId) -> CoverageStatus {
        let summary = ledger.summary(capability);
        let trials = summary.trials();
        let qualifying = summary
            .clusters
            .iter()
            .filter(|c| c.trials >= self.min_trials_per_parent.max(1))
            .count();

        let mut shortfalls = Vec::new();
        if trials < self.min_trials_per_capability {
            shortfalls.push(Shortfall::Trials {
                have: trials,
                need: self.min_trials_per_capability,
            });
        }
        if qualifying < self.min_parents_per_capability {
            shortfalls.push(Shortfall::QualifyingParents {
                have: qualifying,
                need: self.min_parents_per_capability,
            });
        }
        for sentinel in self.sentinels_for(capability) {
            let have = ledger.parent_trials(capability, sentinel);
            let need = self.min_trials_per_parent.max(1);
            if have < need {
                shortfalls.push(Shortfall::Sentinel {
                    parent: sentinel.clone(),
                    have,
                    need,
                });
            }
        }
        if let Some(cap) = self.max_parent_share {
            if trials > 0 {
                for cluster in &summary.clusters {
                    let share = cluster.trials as f64 / trials as f64;
                    if share > cap {
                        shortfalls.push(Shortfall::ParentDominance {
                            parent: cluster.parent.clone(),
                            share,
                            cap,
                        });
                    }
                }
            }
        }

        CoverageStatus {
            capability: capability.clone(),
            trials,
            parents: summary.parents(),
            qualifying_parents: qualifying,
            abstentions: ledger.abstentions(capability),
            shortfalls,
        }
    }
}

/// What coverage a capability has, and what it still lacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageStatus {
    pub capability: CapabilityId,
    pub trials: usize,
    pub parents: usize,
    /// Parents that individually reached `min_trials_per_parent`.
    pub qualifying_parents: usize,
    /// Abstained trials. Recorded, costed, and counted toward nothing.
    pub abstentions: usize,
    pub shortfalls: Vec<Shortfall>,
}

impl CoverageStatus {
    pub fn met(&self) -> bool {
        self.shortfalls.is_empty()
    }

    pub fn describe(&self) -> String {
        if self.met() {
            format!(
                "{} scored trials across {} parents ({} qualifying); all floors met",
                self.trials, self.parents, self.qualifying_parents
            )
        } else {
            self.shortfalls
                .iter()
                .map(Shortfall::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        }
    }

    /// Fails closed with the shortfalls named, for callers that want the error directly.
    pub fn require_met(&self) -> Result<(), AdaptiveError> {
        if self.met() {
            Ok(())
        } else {
            Err(AdaptiveError::CoverageFloorNotMet {
                capability: self.capability.to_string(),
                shortfalls: self.describe(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::InstanceId;
    use crate::ledger::{Outcome, Trial};

    fn ledger_from(spec: &[(&str, usize)]) -> TrialLedger {
        let mut ledger = TrialLedger::new();
        let mut n = 0;
        for (parent, trials) in spec {
            for _ in 0..*trials {
                n += 1;
                ledger
                    .record(
                        Trial::new(
                            CapabilityId::parse("cap").unwrap(),
                            InstanceId::parse(format!("i{n:04}")).unwrap(),
                            ParentId::parse(*parent).unwrap(),
                            if n % 3 == 0 { Outcome::Fail } else { Outcome::Pass },
                            1.0,
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
        }
        ledger
    }

    fn cap() -> CapabilityId {
        CapabilityId::parse("cap").unwrap()
    }

    #[test]
    fn a_capability_below_its_trial_floor_reports_a_named_shortfall() {
        let ledger = ledger_from(&[("p1", 3), ("p2", 3), ("p3", 3), ("p4", 3), ("p5", 3)]);
        let status = CoveragePolicy::default().status(&ledger, &cap());
        assert!(!status.met());
        assert!(status
            .shortfalls
            .contains(&Shortfall::Trials { have: 15, need: 30 }));
        assert!(matches!(
            status.require_met(),
            Err(AdaptiveError::CoverageFloorNotMet { .. })
        ));
    }

    #[test]
    fn one_parent_with_many_trials_cannot_satisfy_the_parent_floor() {
        let ledger = ledger_from(&[("p1", 200)]);
        let status = CoveragePolicy::default().status(&ledger, &cap());
        assert!(!status.met());
        assert!(status.shortfalls.contains(&Shortfall::QualifyingParents {
            have: 1,
            need: 5
        }));
    }

    #[test]
    fn parents_below_the_per_parent_floor_do_not_count_toward_the_parent_floor() {
        // Six parents, but five of them contributed a single trial each.
        let ledger = ledger_from(&[
            ("p1", 40),
            ("p2", 1),
            ("p3", 1),
            ("p4", 1),
            ("p5", 1),
            ("p6", 1),
        ]);
        let status = CoveragePolicy::default().status(&ledger, &cap());
        assert_eq!(status.parents, 6);
        assert_eq!(status.qualifying_parents, 1);
        assert!(!status.met());
    }

    #[test]
    fn a_missing_sentinel_parent_blocks_reporting_however_much_else_was_run() {
        let mut policy = CoveragePolicy::default();
        policy.sentinels.insert(
            cap(),
            [ParentId::parse("golden-1").unwrap()].into_iter().collect(),
        );
        let ledger = ledger_from(&[("p1", 20), ("p2", 20), ("p3", 20), ("p4", 20), ("p5", 20)]);
        let status = policy.status(&ledger, &cap());
        assert!(!status.met());
        assert!(status
            .shortfalls
            .iter()
            .any(|s| matches!(s, Shortfall::Sentinel { .. })));
    }

    #[test]
    fn parent_dominance_is_only_enforced_when_the_policy_asks_for_it() {
        let ledger = ledger_from(&[("p1", 90), ("p2", 3), ("p3", 3), ("p4", 3), ("p5", 3)]);
        let permissive = CoveragePolicy::default();
        assert!(permissive.status(&ledger, &cap()).met());

        let strict = CoveragePolicy {
            max_parent_share: Some(0.5),
            ..CoveragePolicy::default()
        };
        let status = strict.status(&ledger, &cap());
        assert!(!status.met());
        assert!(status
            .shortfalls
            .iter()
            .any(|s| matches!(s, Shortfall::ParentDominance { .. })));
    }

    #[test]
    fn abstentions_do_not_count_toward_any_floor() {
        let mut ledger = TrialLedger::new();
        for i in 0..100 {
            ledger
                .record(
                    Trial::new(
                        cap(),
                        InstanceId::parse(format!("i{i:03}")).unwrap(),
                        ParentId::parse(format!("p{}", i % 10)).unwrap(),
                        Outcome::Abstained,
                        1.0,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let status = CoveragePolicy::default().status(&ledger, &cap());
        assert_eq!(status.trials, 0);
        assert_eq!(status.abstentions, 100);
        assert!(!status.met());
    }
}
