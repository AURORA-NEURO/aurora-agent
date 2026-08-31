//! The routing policy: choose from evidence, or decline to choose.
//!
//! Blueprint 09.08 puts it as "optimize expected task utility minus cost, latency, and risk
//! penalties, with hard policy constraints and safety floors", and adds that "uncertainty drives
//! exploration or conservative fallback". The second clause is the one this module takes
//! seriously, because it is the one a benchmark will not punish you for skipping: a policy that
//! always guesses can still look good on average while being untrustworthy on exactly the tasks
//! where it has no evidence.
//!
//! ## What the policy is
//!
//! A weighted-nearest-neighbour average over prior outcomes. For a new fingerprint it takes every
//! prior observation within [`RoutingPolicy::neighbourhood_radius`], groups by architecture, and
//! ranks by mean utility. There is **no training, no gradient step, no model fitting and no
//! bandit**; blueprint 09.08's "hierarchical outcome models or contextual bandits" and 09.11's
//! online-learning policy are not implemented. Calling this a "learned router" would oversell it,
//! so the report calls it the *evidence router* and this paragraph exists to prevent the drift.
//!
//! ## When it refuses
//!
//! Three conditions force abstention, and each is a distinct reason the reader should be able to
//! tell apart:
//!
//! 1. fewer than two architectures have enough neighbouring observations — routing between one
//!    option is not routing;
//! 2. an architecture is supported by too few *distinct tasks*, so its mean is one task repeated;
//! 3. the top two architectures are separated by less than [`RoutingPolicy::min_margin`], so the
//!    ranking is noise.
//!
//! On abstention the decision names [`RoutingPolicy::safe_default`] and reports confidence
//! exactly zero.
//!
//! ## Choosing the safe default
//!
//! Two choices are sensible and they mean different things. Setting it to the deployment's
//! existing fixed default makes abstention cost nothing relative to not routing at all, which is
//! what [`RoutingPolicy::defaulting_to`] does. Setting it to an architecture that is
//! [`Architecture::admissible_by_construction`] buys a hard safety floor at maximum context cost,
//! which is what [`RoutingPolicy::conservative`] does. The crate does not choose for you, and the
//! regret account will show what the choice cost.

use crate::architecture::{ApprovedSet, Architecture};
use crate::error::RoutingError;
use crate::evidence::EvidenceLedger;
use crate::fingerprint::Fingerprint;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Default structural radius for the neighbourhood.
///
/// Chosen against the axis weights in [`crate::fingerprint`] so that two tasks disagreeing on one
/// categorical axis are never neighbours, while tasks differing only in scale are. Both halves of
/// that statement are pinned by tests in `crate::fingerprint`.
pub const DEFAULT_NEIGHBOURHOOD_RADIUS: f64 = 0.15;

/// Default utility gap below which the top two architectures are treated as tied.
///
/// One percent of the world: an architecture that exposes one fact in a hundred fewer than
/// another has not demonstrated anything worth switching for.
pub const DEFAULT_MIN_MARGIN: f64 = 0.01;

/// Margin at which the margin component of confidence saturates.
pub const DEFAULT_CONFIDENCE_MARGIN_SCALE: f64 = 0.10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingPolicy {
    pub approved: ApprovedSet,
    /// Where the policy goes when it declines to choose.
    pub safe_default: Architecture,
    pub neighbourhood_radius: f64,
    /// Minimum neighbouring observations before an architecture is eligible.
    pub min_observations_per_architecture: usize,
    /// Minimum *distinct* neighbouring tasks before an architecture is eligible.
    pub min_distinct_tasks: usize,
    pub min_margin: f64,
    pub confidence_margin_scale: f64,
}

impl RoutingPolicy {
    /// Falls back to the deployment's existing default, so abstaining costs nothing relative to
    /// not routing at all.
    pub fn defaulting_to(
        approved: ApprovedSet,
        safe_default: Architecture,
    ) -> Result<Self, RoutingError> {
        approved.require(&safe_default)?;
        let policy = RoutingPolicy {
            approved,
            safe_default,
            neighbourhood_radius: DEFAULT_NEIGHBOURHOOD_RADIUS,
            min_observations_per_architecture: 2,
            min_distinct_tasks: 2,
            min_margin: DEFAULT_MIN_MARGIN,
            confidence_margin_scale: DEFAULT_CONFIDENCE_MARGIN_SCALE,
        };
        policy.validate()?;
        Ok(policy)
    }

    /// Falls back to an architecture that cannot drop a decisive witness, whatever the world
    /// looks like. The safety floor is real and so is its cost.
    pub fn conservative(approved: ApprovedSet) -> Result<Self, RoutingError> {
        let floor = approved
            .iter()
            .find(|architecture| architecture.admissible_by_construction())
            .cloned()
            .ok_or(RoutingError::UnapprovedArchitecture {
                architecture: Architecture::FullContext.label(),
            })?;
        RoutingPolicy::defaulting_to(approved, floor)
    }

    pub fn validate(&self) -> Result<(), RoutingError> {
        self.approved.require(&self.safe_default)?;
        check_range("neighbourhood_radius", self.neighbourhood_radius, 0.0, 1.0)?;
        check_range("min_margin", self.min_margin, 0.0, 2.0)?;
        if self.min_observations_per_architecture == 0 {
            return Err(RoutingError::InvalidParameter {
                field: "min_observations_per_architecture",
                value: 0.0,
            });
        }
        if self.min_distinct_tasks == 0 {
            return Err(RoutingError::InvalidParameter {
                field: "min_distinct_tasks",
                value: 0.0,
            });
        }
        if !(self.confidence_margin_scale.is_finite() && self.confidence_margin_scale > 0.0) {
            return Err(RoutingError::InvalidParameter {
                field: "confidence_margin_scale",
                value: self.confidence_margin_scale,
            });
        }
        Ok(())
    }

    /// Choose an architecture for a fingerprint, given evidence about other tasks.
    ///
    /// The caller is responsible for having removed the task being routed from `evidence`. Where
    /// the task identity is known, prefer [`RoutingPolicy::route_unseen`], which checks.
    pub fn route(
        &self,
        fingerprint: &Fingerprint,
        evidence: &EvidenceLedger,
    ) -> Result<RoutingDecision, RoutingError> {
        self.validate()?;
        fingerprint.validate()?;

        let neighbourhood = evidence.neighbourhood(fingerprint, self.neighbourhood_radius);
        let mut considered: Vec<ArchitectureScore> = Vec::new();
        for architecture in self.approved.iter() {
            let matching: Vec<_> = neighbourhood
                .iter()
                .filter(|observation| &observation.architecture == architecture)
                .collect();
            if matching.is_empty() {
                continue;
            }
            let tasks: BTreeSet<&str> = matching
                .iter()
                .map(|observation| observation.task_id.as_str())
                .collect();
            let admissible = matching
                .iter()
                .filter(|observation| observation.admissible())
                .count();
            let total: f64 = matching
                .iter()
                .map(|observation| observation.utility())
                .sum();
            considered.push(ArchitectureScore {
                architecture: architecture.clone(),
                observations: matching.len(),
                distinct_tasks: tasks.len(),
                mean_utility: total / matching.len() as f64,
                admissible_rate: admissible as f64 / matching.len() as f64,
            });
        }

        let mut eligible: Vec<&ArchitectureScore> = considered
            .iter()
            .filter(|score| {
                score.observations >= self.min_observations_per_architecture
                    && score.distinct_tasks >= self.min_distinct_tasks
            })
            .collect();
        eligible.sort_by(|left, right| {
            right
                .mean_utility
                .partial_cmp(&left.mean_utility)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.architecture.label().cmp(&right.architecture.label()))
        });

        if eligible.len() < 2 {
            return Ok(self.abstain(
                DecisionReason::InsufficientCoverage {
                    eligible_architectures: eligible.len(),
                    neighbouring_observations: neighbourhood.len(),
                },
                considered,
            ));
        }

        let best = eligible[0];
        let margin = best.mean_utility - eligible[1].mean_utility;
        if margin < self.min_margin {
            return Ok(self.abstain(
                DecisionReason::InsufficientMargin {
                    margin,
                    runner_up: eligible[1].architecture.clone(),
                },
                considered,
            ));
        }

        let confidence = self.confidence(margin, best.distinct_tasks);
        Ok(RoutingDecision {
            architecture: best.architecture.clone(),
            confidence,
            abstained: false,
            reason: DecisionReason::Routed {
                margin,
                supporting_tasks: best.distinct_tasks,
                runner_up: eligible[1].architecture.clone(),
            },
            considered,
        })
    }

    /// [`RoutingPolicy::route`] with the honesty check attached.
    ///
    /// Refuses if the evidence contains the task being routed. That configuration produces a
    /// number that looks like routing and is actually retrospective selection, and it fails
    /// silently — so it fails loudly here instead.
    pub fn route_unseen(
        &self,
        task_id: &str,
        fingerprint: &Fingerprint,
        evidence: &EvidenceLedger,
    ) -> Result<RoutingDecision, RoutingError> {
        self.validate()?;
        crate::evidence::validate_task_id(task_id)?;
        if evidence.contains_task(task_id) {
            return Err(RoutingError::EvidenceLeak {
                task: task_id.to_string(),
            });
        }
        self.route(fingerprint, evidence)
    }

    fn abstain(
        &self,
        reason: DecisionReason,
        considered: Vec<ArchitectureScore>,
    ) -> RoutingDecision {
        RoutingDecision {
            architecture: self.safe_default.clone(),
            confidence: 0.0,
            abstained: true,
            reason,
            considered,
        }
    }

    /// A monotone evidence score in `[0, 1]`, **not** a posterior probability.
    ///
    /// It rises with the gap to the runner-up and with the number of distinct supporting tasks,
    /// and it is exactly zero when the policy abstained. Whether it is *calibrated* is a question
    /// about measured outcomes, so [`crate::calibration`] measures it against the oracle's choice
    /// rather than this module asserting it.
    fn confidence(&self, margin: f64, supporting_tasks: usize) -> f64 {
        let margin_term = (margin / self.confidence_margin_scale).clamp(0.0, 1.0);
        let floor = self.min_distinct_tasks.max(1) as f64;
        let support_term = supporting_tasks as f64 / (supporting_tasks as f64 + floor);
        margin_term * support_term
    }
}

fn check_range(field: &'static str, value: f64, low: f64, high: f64) -> Result<(), RoutingError> {
    if value.is_finite() && value >= low && value <= high {
        return Ok(());
    }
    Err(RoutingError::InvalidParameter { field, value })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureScore {
    pub architecture: Architecture,
    pub observations: usize,
    pub distinct_tasks: usize,
    pub mean_utility: f64,
    pub admissible_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum DecisionReason {
    Routed {
        margin: f64,
        supporting_tasks: usize,
        runner_up: Architecture,
    },
    /// Fewer than two architectures cleared the support thresholds.
    InsufficientCoverage {
        eligible_architectures: usize,
        neighbouring_observations: usize,
    },
    /// The ranking existed but was inside the noise band.
    InsufficientMargin {
        margin: f64,
        runner_up: Architecture,
    },
}

impl DecisionReason {
    pub fn is_abstention(&self) -> bool {
        !matches!(self, DecisionReason::Routed { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub architecture: Architecture,
    pub confidence: f64,
    pub abstained: bool,
    pub reason: DecisionReason,
    /// Every architecture with at least one neighbouring observation, eligible or not, so a
    /// reader can see what the policy was choosing between rather than only what it chose.
    pub considered: Vec<ArchitectureScore>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{observation_with, synthetic_fingerprint};

    fn panel() -> ApprovedSet {
        ApprovedSet::new([
            Architecture::FullContext,
            Architecture::FiberCompiled,
            Architecture::LexicalTopK { k: 11 },
        ])
        .unwrap()
    }

    fn policy() -> RoutingPolicy {
        RoutingPolicy::defaulting_to(panel(), Architecture::FullContext).unwrap()
    }

    #[test]
    fn a_router_with_insufficient_evidence_abstains_to_the_default() {
        let fingerprint = synthetic_fingerprint(1.0, 1, false, 100);
        let empty = EvidenceLedger::new([]).unwrap();
        let decision = policy().route(&fingerprint, &empty).unwrap();

        assert!(decision.abstained);
        assert_eq!(decision.architecture, Architecture::FullContext);
        assert_eq!(decision.confidence, 0.0);
        assert!(matches!(
            decision.reason,
            DecisionReason::InsufficientCoverage { .. }
        ));
    }

    #[test]
    fn a_router_abstains_when_only_one_architecture_has_support() {
        let fingerprint = synthetic_fingerprint(1.0, 1, false, 100);
        let evidence = EvidenceLedger::new([
            observation_with(
                &fingerprint,
                "a",
                Architecture::FiberCompiled,
                true,
                10,
                100,
            ),
            observation_with(
                &fingerprint,
                "b",
                Architecture::FiberCompiled,
                true,
                10,
                100,
            ),
        ])
        .unwrap();

        let decision = policy().route(&fingerprint, &evidence).unwrap();
        assert!(
            decision.abstained,
            "routing between one option is not routing"
        );
        assert!(matches!(
            decision.reason,
            DecisionReason::InsufficientCoverage {
                eligible_architectures: 1,
                ..
            }
        ));
    }

    #[test]
    fn a_router_abstains_when_the_top_two_architectures_are_inside_the_noise_band() {
        let fingerprint = synthetic_fingerprint(1.0, 1, false, 100);
        let evidence = EvidenceLedger::new([
            observation_with(
                &fingerprint,
                "a",
                Architecture::FiberCompiled,
                true,
                10,
                100,
            ),
            observation_with(
                &fingerprint,
                "b",
                Architecture::FiberCompiled,
                true,
                10,
                100,
            ),
            observation_with(
                &fingerprint,
                "a",
                Architecture::LexicalTopK { k: 11 },
                true,
                10,
                100,
            ),
            observation_with(
                &fingerprint,
                "b",
                Architecture::LexicalTopK { k: 11 },
                true,
                11,
                100,
            ),
        ])
        .unwrap();

        let decision = policy().route(&fingerprint, &evidence).unwrap();
        assert!(decision.abstained);
        assert!(matches!(
            decision.reason,
            DecisionReason::InsufficientMargin { .. }
        ));
        assert_eq!(decision.architecture, Architecture::FullContext);
    }

    #[test]
    fn a_router_with_a_clear_separation_picks_the_better_architecture() {
        let fingerprint = synthetic_fingerprint(1.0, 1, false, 100);
        let evidence = EvidenceLedger::new([
            observation_with(
                &fingerprint,
                "a",
                Architecture::FiberCompiled,
                true,
                10,
                100,
            ),
            observation_with(
                &fingerprint,
                "b",
                Architecture::FiberCompiled,
                true,
                10,
                100,
            ),
            observation_with(&fingerprint, "a", Architecture::FullContext, true, 100, 100),
            observation_with(&fingerprint, "b", Architecture::FullContext, true, 100, 100),
        ])
        .unwrap();

        let decision = policy().route(&fingerprint, &evidence).unwrap();
        assert!(!decision.abstained);
        assert_eq!(decision.architecture, Architecture::FiberCompiled);
        assert!(decision.confidence > 0.0);
    }

    #[test]
    fn evidence_from_a_structurally_different_task_does_not_enter_the_neighbourhood() {
        let here = synthetic_fingerprint(1.0, 1, false, 100);
        let elsewhere = synthetic_fingerprint(0.0, 4, true, 100);
        let evidence = EvidenceLedger::new([
            observation_with(&elsewhere, "a", Architecture::FiberCompiled, true, 10, 100),
            observation_with(&elsewhere, "b", Architecture::FiberCompiled, true, 10, 100),
            observation_with(&elsewhere, "a", Architecture::FullContext, true, 100, 100),
            observation_with(&elsewhere, "b", Architecture::FullContext, true, 100, 100),
        ])
        .unwrap();

        let decision = policy().route(&here, &evidence).unwrap();
        assert!(decision.abstained);
        assert!(decision.considered.is_empty());
    }

    #[test]
    fn routing_a_task_whose_own_outcome_is_in_the_evidence_is_refused() {
        let fingerprint = synthetic_fingerprint(1.0, 1, false, 100);
        let evidence = EvidenceLedger::new([observation_with(
            &fingerprint,
            "held-out",
            Architecture::FiberCompiled,
            true,
            10,
            100,
        )])
        .unwrap();

        let error = policy()
            .route_unseen("held-out", &fingerprint, &evidence)
            .unwrap_err();
        assert_eq!(
            error,
            RoutingError::EvidenceLeak {
                task: "held-out".to_string()
            }
        );

        assert!(matches!(
            policy()
                .route_unseen(" held-out", &fingerprint, &evidence)
                .unwrap_err(),
            RoutingError::InvalidTaskId { .. }
        ));
    }

    #[test]
    fn confidence_rises_with_the_margin_and_with_the_number_of_supporting_tasks() {
        let policy = policy();
        let narrow = policy.confidence(0.02, 4);
        let wide = policy.confidence(0.08, 4);
        let thin = policy.confidence(0.08, 2);

        assert!(wide > narrow, "a larger gap must not lower confidence");
        assert!(
            wide > thin,
            "more supporting tasks must not lower confidence"
        );
        assert!((0.0..=1.0).contains(&wide));
    }

    #[test]
    fn the_policy_never_returns_an_architecture_outside_the_approved_set() {
        let fingerprint = synthetic_fingerprint(1.0, 1, false, 100);
        let outsider = synthetic_fingerprint(1.0, 1, false, 100);
        let evidence = EvidenceLedger::new([
            observation_with(
                &outsider,
                "a",
                Architecture::GraphKHop { depth: 5 },
                true,
                4,
                100,
            ),
            observation_with(
                &outsider,
                "b",
                Architecture::GraphKHop { depth: 5 },
                true,
                4,
                100,
            ),
            observation_with(&outsider, "a", Architecture::FullContext, true, 100, 100),
            observation_with(&outsider, "b", Architecture::FullContext, true, 100, 100),
        ])
        .unwrap();

        let decision = policy().route(&fingerprint, &evidence).unwrap();
        assert!(
            panel().contains(&decision.architecture),
            "an unreviewed architecture leaked into a decision"
        );
        assert!(
            decision
                .considered
                .iter()
                .all(|score| panel().contains(&score.architecture)),
            "an unapproved architecture was scored"
        );
    }

    #[test]
    fn a_conservative_policy_abstains_to_an_architecture_that_cannot_drop_a_witness() {
        let conservative = RoutingPolicy::conservative(panel()).unwrap();
        assert!(conservative.safe_default.admissible_by_construction());

        let missing_floor =
            ApprovedSet::new([Architecture::FiberCompiled, Architecture::QueryGraph]).unwrap();
        assert!(RoutingPolicy::conservative(missing_floor).is_err());
    }

    #[test]
    fn an_out_of_range_policy_parameter_is_refused_rather_than_clamped() {
        let mut policy = policy();
        policy.neighbourhood_radius = 4.0;
        assert_eq!(
            policy.validate().unwrap_err(),
            RoutingError::InvalidParameter {
                field: "neighbourhood_radius",
                value: 4.0
            }
        );
    }

    #[test]
    fn zero_support_thresholds_are_refused() {
        let mut zero_observations_policy = policy();
        zero_observations_policy.min_observations_per_architecture = 0;
        assert_eq!(
            zero_observations_policy.validate().unwrap_err(),
            RoutingError::InvalidParameter {
                field: "min_observations_per_architecture",
                value: 0.0
            }
        );

        let mut zero_distinct_policy = policy();
        zero_distinct_policy.min_distinct_tasks = 0;
        assert_eq!(
            zero_distinct_policy.validate().unwrap_err(),
            RoutingError::InvalidParameter {
                field: "min_distinct_tasks",
                value: 0.0
            }
        );

        let fingerprint = synthetic_fingerprint(1.0, 1, false, 100);
        let evidence = EvidenceLedger::new([observation_with(
            &fingerprint,
            "held-out",
            Architecture::FiberCompiled,
            true,
            10,
            100,
        )])
        .unwrap();
        let mut invalid_policy = policy();
        invalid_policy.min_distinct_tasks = 0;
        assert!(matches!(
            invalid_policy
                .route_unseen("held-out", &fingerprint, &evidence)
                .unwrap_err(),
            RoutingError::InvalidParameter {
                field: "min_distinct_tasks",
                value: 0.0
            }
        ));
    }

    #[test]
    fn a_decision_round_trips_through_json() {
        let fingerprint = synthetic_fingerprint(1.0, 1, false, 100);
        let empty = EvidenceLedger::new([]).unwrap();
        let decision = policy().route(&fingerprint, &empty).unwrap();
        let text = serde_json::to_string(&decision).unwrap();
        assert_eq!(
            serde_json::from_str::<RoutingDecision>(&text).unwrap(),
            decision
        );
    }
}
