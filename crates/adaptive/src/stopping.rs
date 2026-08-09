//! When to stop, and what to say about why.
//!
//! Blueprint 08.05 lists the legitimate stopping targets — "Capability interval width; posterior
//! probability of a release decision; ...; budget exhaustion" — and attaches two conditions that
//! this module enforces literally.
//!
//! **"Even decisive early results must satisfy minimum independent-parent and required-strata
//! counts."** A run may not stop with a conclusion while a coverage floor is unmet. It may stop
//! *without* one, by exhausting its budget, and then it reports inconclusive.
//!
//! **"Stop when remaining budget cannot plausibly resolve the decision, and report inconclusive
//! rather than forcing a verdict."** That is [`StopReason::Futile`].
//!
//! Every verdict reports the **effective** sample size next to the raw count, because the raw
//! count is what a reader will otherwise assume the confidence came from.
//!
//! # What this is not
//!
//! It is not anytime-valid. 08.05 asks for "anytime-valid confidence sequences, or
//! alpha-spending/sequential tests appropriate to the estimand"; what is implemented is a fixed
//! Bayesian credible interval checked repeatedly. Under repeated looks that interval's frequentist
//! coverage is *below* its nominal level — optional stopping is real, and a panel that peeks after
//! every trial and stops the moment the width target is hit will be optimistically biased. Two
//! things blunt it here and neither removes it: the interval being checked is already inflated by
//! the design effect, and the minimum-effective-trials floor forbids stopping in the regime where
//! the bias is worst. A confidence sequence (a mixture martingale or a beta-binomial e-process)
//! is the correct fix and is not implemented. Do not treat a stopped panel's interval as having
//! exact 95% frequentist coverage.

use crate::beta::{BetaPosterior, BetaPrior, CredibleInterval};
use crate::cluster::ClusterSummary;
use crate::coverage::CoverageStatus;
use crate::error::AdaptiveError;
use crate::id::CapabilityId;
use serde::{Deserialize, Serialize};

/// The question the panel is being run to answer.
///
/// Stopping is defined relative to a question. A panel with no question cannot be finished, only
/// out of money — which is 08.05's point that "Stopping is an explicit decision, not model
/// fatigue" (43.15).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Question {
    /// "Pin the capability down to within `target`." Stops on clustered interval width.
    IntervalWidth { target: f64 },
    /// "Is the capability above `theta`?" Stops when the clustered posterior puts at least
    /// `confidence` mass on one side.
    Threshold { theta: f64, confidence: f64 },
}

impl Default for Question {
    fn default() -> Self {
        Question::IntervalWidth { target: 0.10 }
    }
}

impl Question {
    fn answered(&self, posterior: &BetaPosterior, interval: &CredibleInterval) -> bool {
        match self {
            Question::IntervalWidth { target } => interval.width() <= *target,
            Question::Threshold { theta, confidence } => {
                let above = 1.0 - posterior.cdf(*theta);
                above.max(1.0 - above) >= *confidence
            }
        }
    }
}

/// Budget and evidence floors for one capability.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StoppingRule {
    pub question: Question,
    /// Maximum scored trials for this capability. Gate 4's panel sizes are 500–2,000 *in total*,
    /// so this is a per-capability slice of that.
    pub budget: usize,
    /// The panel may not conclude below this many *effective* trials, however narrow the interval
    /// happens to look. This is the floor that stops a single homogeneous parent family from
    /// closing a question on its own.
    pub min_effective_trials: f64,
    pub credibility: f64,
}

impl Default for StoppingRule {
    fn default() -> Self {
        StoppingRule {
            question: Question::default(),
            budget: 400,
            min_effective_trials: 10.0,
            credibility: 0.95,
        }
    }
}

/// Why the panel stopped, or why it has not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Continue: a coverage floor is still unmet.
    CoverageIncomplete,
    /// Continue: the question is not yet answered and the budget can still plausibly answer it.
    EvidenceInsufficient,
    /// Stop, conclusive: the clustered interval reached the target width.
    TargetWidthReached,
    /// Stop, conclusive: the threshold question resolved.
    DecisionResolved,
    /// Stop, inconclusive: the budget ran out first.
    BudgetExhausted,
    /// Stop, inconclusive: the remaining budget cannot answer the question even in the best case.
    Futile,
}

impl StopReason {
    pub fn stops(self) -> bool {
        !matches!(
            self,
            StopReason::CoverageIncomplete | StopReason::EvidenceInsufficient
        )
    }

    /// Whether the panel is entitled to state an answer.
    pub fn is_conclusive(self) -> bool {
        matches!(
            self,
            StopReason::TargetWidthReached | StopReason::DecisionResolved
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::CoverageIncomplete => "coverage_incomplete",
            StopReason::EvidenceInsufficient => "evidence_insufficient",
            StopReason::TargetWidthReached => "target_width_reached",
            StopReason::DecisionResolved => "decision_resolved",
            StopReason::BudgetExhausted => "budget_exhausted",
            StopReason::Futile => "futile",
        }
    }
}

/// The stopping decision, with the evidence that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoppingVerdict {
    pub capability: CapabilityId,
    pub reason: StopReason,
    pub stop: bool,
    pub conclusive: bool,
    /// Scored trials run. The number a report should *not* lead with.
    pub trials: usize,
    /// What those trials were worth after the parent-clustering correction.
    pub effective_trials: f64,
    pub design_effect: f64,
    pub remaining_budget: usize,
    /// The interval the decision was taken on: clustered, not naive.
    pub interval: CredibleInterval,
    /// An upper bound on how narrow the interval could get by spending the whole remaining
    /// budget on fresh parents. See [`best_case_effective_trials`].
    pub best_case_width: f64,
    pub detail: String,
}

/// An upper bound on the effective sample size the remaining budget can buy.
///
/// Every remaining trial is assumed to come from a previously unseen parent, which is the most
/// informative thing it could be. Writing `n_eff = n^2 / D` with
/// `D = n(1 - rho) + rho * sum(m^2)`, adding `r` singleton clusters gives
/// `(n+r)^2 / (D+r)`, and `(n+r)^2/(D+r) <= n^2/D + r` reduces to `0 <= (n - D)^2`. So the bound
/// below is genuinely a bound and not an approximation — futility declared against it cannot be
/// undone by better luck with parents.
pub fn best_case_effective_trials(summary: &ClusterSummary, remaining: usize) -> f64 {
    summary.effective_trials() + remaining as f64
}

/// Evaluates the stopping rule for one capability.
///
/// The order of the checks is the contract: a conclusion requires both coverage and the effective
/// evidence floor; budget exhaustion overrides everything else that would keep the panel running;
/// futility is only declared once the panel is otherwise willing to continue.
pub fn evaluate(
    capability: &CapabilityId,
    summary: &ClusterSummary,
    coverage: &CoverageStatus,
    prior: &BetaPrior,
    rule: &StoppingRule,
) -> Result<StoppingVerdict, AdaptiveError> {
    let posterior = summary.clustered_posterior(prior)?;
    let interval = posterior.interval(rule.credibility)?;
    let trials = summary.trials();
    let effective = summary.effective_trials();
    let remaining = rule.budget.saturating_sub(trials);

    let best_case = posterior
        .with_mass(prior.mass() + best_case_effective_trials(summary, remaining))?
        .interval(rule.credibility)?;

    let answered = rule.question.answered(&posterior, &interval);
    let floors_met = coverage.met() && effective >= rule.min_effective_trials;

    let (reason, detail) = if answered && floors_met {
        let reason = match rule.question {
            Question::IntervalWidth { .. } => StopReason::TargetWidthReached,
            Question::Threshold { .. } => StopReason::DecisionResolved,
        };
        (
            reason,
            format!(
                "answered on {effective:.1} effective trials from {} raw ({} parents)",
                trials, coverage.parents
            ),
        )
    } else if remaining == 0 {
        (
            StopReason::BudgetExhausted,
            format!(
                "budget of {} scored trials spent; {}",
                rule.budget,
                if coverage.met() {
                    "question unresolved".to_string()
                } else {
                    coverage.describe()
                }
            ),
        )
    } else if !rule
        .question
        .answered(&posterior.with_mass(prior.mass() + best_case_effective_trials(summary, remaining))?, &best_case)
    {
        (
            StopReason::Futile,
            format!(
                "spending the remaining {remaining} trials on entirely fresh parents could reach \
                 a width of only {:.3}; reporting inconclusive rather than a verdict",
                best_case.width()
            ),
        )
    } else if !coverage.met() {
        (StopReason::CoverageIncomplete, coverage.describe())
    } else {
        (
            StopReason::EvidenceInsufficient,
            format!(
                "{effective:.1} effective trials so far (floor {:.1}), interval width {:.3}",
                rule.min_effective_trials,
                interval.width()
            ),
        )
    };

    Ok(StoppingVerdict {
        capability: capability.clone(),
        reason,
        stop: reason.stops(),
        conclusive: reason.is_conclusive(),
        trials,
        effective_trials: effective,
        design_effect: summary.design_effect(),
        remaining_budget: remaining,
        interval,
        best_case_width: best_case.width(),
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::Cluster;
    use crate::coverage::CoveragePolicy;
    use crate::id::{InstanceId, ParentId};
    use crate::ledger::{Outcome, Trial, TrialLedger};

    fn cap() -> CapabilityId {
        CapabilityId::parse("cap").unwrap()
    }

    fn build(spec: &[(usize, usize)]) -> (ClusterSummary, CoverageStatus) {
        let mut ledger = TrialLedger::new();
        let mut n = 0usize;
        for (i, (trials, successes)) in spec.iter().enumerate() {
            for t in 0..*trials {
                n += 1;
                ledger
                    .record(
                        Trial::new(
                            cap(),
                            InstanceId::parse(format!("i{n:05}")).unwrap(),
                            ParentId::parse(format!("p{i:03}")).unwrap(),
                            if t < *successes {
                                Outcome::Pass
                            } else {
                                Outcome::Fail
                            },
                            1.0,
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
        }
        let status = CoveragePolicy::default().status(&ledger, &cap());
        (ledger.summary(&cap()), status)
    }

    fn summary_only(spec: &[(usize, usize)]) -> ClusterSummary {
        ClusterSummary::new(
            spec.iter()
                .enumerate()
                .map(|(i, (trials, successes))| Cluster {
                    parent: ParentId::parse(format!("p{i:03}")).unwrap(),
                    trials: *trials,
                    successes: *successes,
                })
                .collect(),
        )
    }

    #[test]
    fn a_panel_with_an_unmet_coverage_floor_may_not_conclude_however_narrow_its_interval() {
        // One parent, four hundred trials: the naive interval would be about two points wide.
        let (summary, coverage) = build(&[(400, 240)]);
        let verdict = evaluate(
            &cap(),
            &summary,
            &coverage,
            &BetaPrior::default(),
            &StoppingRule {
                budget: 2000,
                ..StoppingRule::default()
            },
        )
        .unwrap();
        assert!(!verdict.conclusive);
        assert_eq!(verdict.reason, StopReason::CoverageIncomplete);
        assert_eq!(verdict.trials, 400);
        assert!(verdict.effective_trials < 2.0);
    }

    #[test]
    fn budget_exhaustion_stops_the_panel_but_reports_inconclusive() {
        let (summary, coverage) = build(&[(6, 3), (6, 4), (6, 2), (6, 5), (6, 3)]);
        let verdict = evaluate(
            &cap(),
            &summary,
            &coverage,
            &BetaPrior::default(),
            &StoppingRule {
                budget: 30,
                question: Question::IntervalWidth { target: 0.02 },
                ..StoppingRule::default()
            },
        )
        .unwrap();
        assert_eq!(verdict.reason, StopReason::BudgetExhausted);
        assert!(verdict.stop);
        assert!(!verdict.conclusive);
        assert_eq!(verdict.remaining_budget, 0);
    }

    #[test]
    fn an_unreachable_width_target_is_declared_futile_rather_than_pursued() {
        let (summary, coverage) = build(&[(10, 6), (10, 5), (10, 7), (10, 4), (10, 6)]);
        let verdict = evaluate(
            &cap(),
            &summary,
            &coverage,
            &BetaPrior::default(),
            &StoppingRule {
                budget: 60,
                question: Question::IntervalWidth { target: 0.001 },
                ..StoppingRule::default()
            },
        )
        .unwrap();
        assert_eq!(verdict.reason, StopReason::Futile);
        assert!(verdict.stop);
        assert!(!verdict.conclusive);
        assert!(verdict.best_case_width > 0.001);
    }

    #[test]
    fn a_well_covered_panel_stops_on_the_width_target_and_reports_effective_trials() {
        let (summary, coverage) = build(&[
            (60, 30),
            (60, 32),
            (60, 28),
            (60, 31),
            (60, 29),
            (60, 30),
            (60, 33),
        ]);
        let verdict = evaluate(
            &cap(),
            &summary,
            &coverage,
            &BetaPrior::default(),
            &StoppingRule {
                budget: 2000,
                question: Question::IntervalWidth { target: 0.12 },
                ..StoppingRule::default()
            },
        )
        .unwrap();
        assert_eq!(verdict.reason, StopReason::TargetWidthReached);
        assert!(verdict.conclusive);
        assert_eq!(verdict.trials, 420);
        // These parents genuinely agree with one another, so the estimated rho is zero and the
        // correction costs nothing. That is the honest outcome, and it is why the design effect
        // is reported rather than assumed: it is not a fixed penalty on generated instances.
        assert!(verdict.design_effect >= 1.0);
        assert!(verdict.effective_trials <= verdict.trials as f64);
        assert!(verdict.effective_trials >= 8.0);
    }

    #[test]
    fn a_threshold_question_resolves_on_posterior_mass_rather_than_width() {
        let (summary, coverage) = build(&[
            (40, 38),
            (40, 37),
            (40, 39),
            (40, 36),
            (40, 38),
            (40, 37),
        ]);
        let verdict = evaluate(
            &cap(),
            &summary,
            &coverage,
            &BetaPrior::default(),
            &StoppingRule {
                budget: 2000,
                question: Question::Threshold {
                    theta: 0.5,
                    confidence: 0.99,
                },
                ..StoppingRule::default()
            },
        )
        .unwrap();
        assert_eq!(verdict.reason, StopReason::DecisionResolved);
        assert!(verdict.interval.lo > 0.5);
    }

    #[test]
    fn the_effective_trial_floor_blocks_a_conclusion_from_a_homogeneous_family() {
        // Three parents, all internally unanimous: rho is unidentifiable and assumed worst case,
        // so the panel is worth three effective trials and may not conclude on the default floor
        // of ten however tempting the raw count is.
        let mut ledger = TrialLedger::new();
        for p in 0..3 {
            for i in 0..100 {
                ledger
                    .record(
                        Trial::new(
                            cap(),
                            InstanceId::parse(format!("i{p}-{i:03}")).unwrap(),
                            ParentId::parse(format!("p{p}")).unwrap(),
                            Outcome::Pass,
                            1.0,
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
        }
        let policy = CoveragePolicy {
            min_trials_per_capability: 30,
            min_parents_per_capability: 3,
            min_trials_per_parent: 2,
            max_parent_share: None,
            sentinels: Default::default(),
        };
        let coverage = policy.status(&ledger, &cap());
        assert!(coverage.met());
        let verdict = evaluate(
            &cap(),
            &ledger.summary(&cap()),
            &coverage,
            &BetaPrior::default(),
            &StoppingRule {
                budget: 2000,
                question: Question::Threshold {
                    theta: 0.5,
                    confidence: 0.9,
                },
                ..StoppingRule::default()
            },
        )
        .unwrap();
        assert!(!verdict.conclusive, "{verdict:?}");
        assert!(verdict.effective_trials < 10.0);
    }

    #[test]
    fn the_text_form_of_a_stop_reason_cannot_drift_from_its_serialized_form() {
        for reason in [
            StopReason::CoverageIncomplete,
            StopReason::EvidenceInsufficient,
            StopReason::TargetWidthReached,
            StopReason::DecisionResolved,
            StopReason::BudgetExhausted,
            StopReason::Futile,
        ] {
            assert_eq!(
                serde_json::to_string(&reason).unwrap(),
                format!("\"{}\"", reason.as_str())
            );
            assert_eq!(reason.is_conclusive(), reason.stops() && !matches!(reason, StopReason::BudgetExhausted | StopReason::Futile));
        }
    }

    #[test]
    fn the_best_case_effective_size_is_an_upper_bound_on_what_fresh_parents_can_buy() {
        let summary = summary_only(&[(50, 30), (50, 25), (30, 20)]);
        let remaining = 40usize;
        let bound = best_case_effective_trials(&summary, remaining);
        // Actually add the fresh singleton parents and confirm the realised effective size never
        // exceeds the bound.
        let mut clusters = summary.clusters.clone();
        for i in 0..remaining {
            clusters.push(Cluster {
                parent: ParentId::parse(format!("fresh-{i:03}")).unwrap(),
                trials: 1,
                successes: i % 2,
            });
        }
        let grown = ClusterSummary::new(clusters);
        assert!(
            grown.effective_trials() <= bound + 1e-9,
            "{} exceeded the bound {}",
            grown.effective_trials(),
            bound
        );
    }
}
