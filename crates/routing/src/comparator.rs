//! The four things Gate 6 requires you to report together.
//!
//! Blueprint `00_START_HERE/04_CRITICAL_PATH.md`, Gate 6: "Use prior evaluation evidence to
//! choose between at least three architecture configurations on unseen tasks. Compare against a
//! fixed default, a largest-model default, and an oracle retrospective selector."
//!
//! The oracle is the load-bearing one and it is the one most often left out, because it is the
//! only comparator that can make a router look bad. Without it, "the router beat the default"
//! is compatible with the router capturing two percent of what was available; with it, the same
//! sentence becomes a fraction anyone can check. This crate therefore treats the oracle as
//! mandatory rather than optional, and [`Comparator::sees_the_answer`] marks it as the one
//! selector that is *not* a deployable policy.
//!
//! The mapping from Gate 6's wording to this workspace:
//!
//! | Gate 6 | here |
//! |---|---|
//! | fixed default | one approved context architecture, chosen up front and used on every task |
//! | largest-model default | the architecture with the highest measured mean context cost, which on any non-empty world is full context |
//! | learned router | [`crate::policy::RoutingPolicy`], a nearest-neighbour average over held-out evidence |
//! | oracle retrospective selector | the best outcome actually observed for each task |
//!
//! "Largest model" becomes "largest context" because this crate routes between context
//! architectures, not models. No model is selected anywhere in this crate and none is called.

use crate::architecture::Architecture;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "comparator", rename_all = "snake_case")]
pub enum Comparator {
    /// The same approved architecture on every task, whatever the fingerprint says.
    FixedDefault { architecture: Architecture },
    /// The most expensive approved architecture on every task.
    MostExpensiveDefault { architecture: Architecture },
    /// The evidence-conditioned policy, run on evidence that excludes the task being routed.
    EvidenceRouter,
    /// The best observed outcome per task. A bound on what any router could achieve, not a policy.
    OracleRetrospective,
}

impl Comparator {
    pub fn name(&self) -> &'static str {
        match self {
            Comparator::FixedDefault { .. } => "fixed-default",
            Comparator::MostExpensiveDefault { .. } => "most-expensive-default",
            Comparator::EvidenceRouter => "evidence-router",
            Comparator::OracleRetrospective => "oracle-retrospective",
        }
    }

    /// Whether this comparator is allowed to know the task's own outcome.
    ///
    /// Exactly one comparator may answer yes. Any other yes is a leak, and the report's
    /// invariant test says so.
    pub fn sees_the_answer(&self) -> bool {
        matches!(self, Comparator::OracleRetrospective)
    }

    /// Whether this comparator could actually be deployed.
    pub fn is_deployable(&self) -> bool {
        !self.sees_the_answer()
    }

    /// The architecture this comparator always picks, where it always picks one.
    pub fn fixed_architecture(&self) -> Option<&Architecture> {
        match self {
            Comparator::FixedDefault { architecture }
            | Comparator::MostExpensiveDefault { architecture } => Some(architecture),
            Comparator::EvidenceRouter | Comparator::OracleRetrospective => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Comparator> {
        vec![
            Comparator::FixedDefault {
                architecture: Architecture::FiberCompiled,
            },
            Comparator::MostExpensiveDefault {
                architecture: Architecture::FullContext,
            },
            Comparator::EvidenceRouter,
            Comparator::OracleRetrospective,
        ]
    }

    #[test]
    fn gate_six_requires_exactly_four_comparators() {
        assert_eq!(all().len(), 4);
    }

    #[test]
    fn exactly_one_comparator_sees_the_answer_and_it_is_the_oracle() {
        let cheating: Vec<&'static str> = all()
            .iter()
            .filter(|comparator| comparator.sees_the_answer())
            .map(Comparator::name)
            .collect();
        assert_eq!(cheating, vec!["oracle-retrospective"]);
    }

    #[test]
    fn the_oracle_is_a_bound_and_is_not_deployable() {
        assert!(!Comparator::OracleRetrospective.is_deployable());
        assert!(Comparator::EvidenceRouter.is_deployable());
    }

    #[test]
    fn every_comparator_name_is_distinct() {
        let mut names: Vec<&'static str> = all().iter().map(Comparator::name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 4);
    }
}
