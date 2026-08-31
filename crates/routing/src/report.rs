//! The report, including the version of it nobody wants to publish.
//!
//! Blueprint 43.41's stop rule requires reporting results that go the wrong way, and
//! `crates/baseline` already does this for context strategies — it pins the measurement where a
//! BM25 retriever ties the compiler. Routing is where the temptation to flatter is strongest,
//! because a router has two free parameters (which default it is compared against, and which
//! tasks are in the panel) and either one can be nudged until the headline reads well.
//!
//! Three structural defences, in the types rather than in a convention:
//!
//! - [`crate::regret::RegretAccount`] has four named fields, so a report cannot be assembled
//!   without the oracle bound;
//! - [`crate::regret::RoutingVerdict`] is computed from the numbers and printed unconditionally,
//!   with "the router lost" checked before any other case;
//! - the markdown always prints the captured-gain fraction beside the win rate, so a large win
//!   rate over a small captured fraction is visible in one line rather than needing to be
//!   inferred.
//!
//! What the report does **not** contain: confidence intervals, significance tests, latency, token
//! cost, provider risk, or any claim about models. None of those are measured by this crate.

use crate::architecture::Architecture;
use crate::calibration::CalibrationCurve;
use crate::comparator::Comparator;
use crate::error::RoutingError;
use crate::evidence::{validate_task_id, Observation, INADMISSIBLE_UTILITY};
use crate::fingerprint::Regime;
use crate::policy::DecisionReason;
use crate::regret::{ComparatorOutcome, RegretAccount, RoutingVerdict, GAIN_EPSILON};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// What one comparator did on one task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparatorPick {
    pub architecture: Architecture,
    pub admissible: bool,
    pub cost_fraction: f64,
    pub utility: f64,
}

impl ComparatorPick {
    pub fn of(observation: &Observation) -> Self {
        ComparatorPick {
            architecture: observation.architecture.clone(),
            admissible: observation.admissible(),
            cost_fraction: observation.cost_fraction(),
            utility: observation.utility(),
        }
    }
}

/// One held-out task, with all four comparators scored on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRow {
    pub task_id: String,
    pub regime: Regime,
    pub facts: usize,
    pub abstained: bool,
    pub confidence: f64,
    pub reason: DecisionReason,
    pub fixed_default: ComparatorPick,
    pub most_expensive_default: ComparatorPick,
    pub router: ComparatorPick,
    pub oracle: ComparatorPick,
}

impl TaskRow {
    /// Shortfall of the router against the retrospective bound on this task.
    pub fn router_regret(&self) -> f64 {
        self.oracle.utility - self.router.utility
    }

    /// Whether the router chose the architecture the oracle would have chosen.
    pub fn matched_oracle(&self) -> bool {
        self.router.architecture == self.oracle.architecture
    }

    pub fn beat_fixed_default(&self) -> bool {
        self.router.utility > self.fixed_default.utility + GAIN_EPSILON
    }

    pub fn lost_to_fixed_default(&self) -> bool {
        self.router.utility < self.fixed_default.utility - GAIN_EPSILON
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingReport {
    pub tasks: Vec<TaskRow>,
    pub account: RegretAccount,
    pub calibration: CalibrationCurve,
    pub verdict: RoutingVerdict,
    pub abstention_rate: f64,
    /// Share of *routed* decisions that matched the oracle's architecture.
    pub oracle_agreement_rate: Option<f64>,
    pub tasks_won: usize,
    pub tasks_lost: usize,
    pub tasks_tied: usize,
    /// Stated limitations, carried in the artefact so they survive copy-paste.
    pub caveats: Vec<String>,
}

impl RoutingReport {
    pub fn assemble(
        tasks: Vec<TaskRow>,
        fixed_default: Architecture,
        most_expensive_default: Architecture,
        calibration_bins: usize,
    ) -> Result<Self, RoutingError> {
        if tasks.is_empty() {
            return Err(RoutingError::NoTasks);
        }
        let mut task_ids = BTreeSet::new();
        for row in &tasks {
            validate_task_row(row)?;
            if !task_ids.insert(row.task_id.to_ascii_lowercase()) {
                return Err(RoutingError::InvalidReport {
                    detail: "task_ids must be unique without case collisions".into(),
                });
            }
        }

        let oracle_utilities: Vec<f64> = tasks.iter().map(|row| row.oracle.utility).collect();
        let account = RegretAccount {
            fixed_default: outcome(
                Comparator::FixedDefault {
                    architecture: fixed_default,
                },
                tasks.iter().map(|row| &row.fixed_default),
                &oracle_utilities,
            ),
            most_expensive_default: outcome(
                Comparator::MostExpensiveDefault {
                    architecture: most_expensive_default,
                },
                tasks.iter().map(|row| &row.most_expensive_default),
                &oracle_utilities,
            ),
            router: outcome(
                Comparator::EvidenceRouter,
                tasks.iter().map(|row| &row.router),
                &oracle_utilities,
            ),
            oracle: outcome(
                Comparator::OracleRetrospective,
                tasks.iter().map(|row| &row.oracle),
                &oracle_utilities,
            ),
        };

        let routed: Vec<(f64, bool)> = tasks
            .iter()
            .filter(|row| !row.abstained)
            .map(|row| (row.confidence, row.matched_oracle()))
            .collect();
        let calibration = CalibrationCurve::build(&routed, calibration_bins)?;

        let total = tasks.len() as f64;
        let abstentions = tasks.iter().filter(|row| row.abstained).count();
        let tasks_won = tasks.iter().filter(|row| row.beat_fixed_default()).count();
        let tasks_lost = tasks
            .iter()
            .filter(|row| row.lost_to_fixed_default())
            .count();

        Ok(RoutingReport {
            verdict: RoutingVerdict::of(&account),
            abstention_rate: abstentions as f64 / total,
            oracle_agreement_rate: if routed.is_empty() {
                None
            } else {
                Some(
                    routed.iter().filter(|(_, matched)| *matched).count() as f64
                        / routed.len() as f64,
                )
            },
            tasks_won,
            tasks_lost,
            tasks_tied: tasks.len() - tasks_won - tasks_lost,
            caveats: default_caveats(),
            tasks,
            account,
            calibration,
        })
    }

    /// Share of tasks where the router strictly beat the fixed default.
    ///
    /// Reported only alongside [`RegretAccount::captured_gain_fraction`]. On its own it is the
    /// statistic that makes a bad router look good.
    pub fn win_rate(&self) -> f64 {
        self.tasks_won as f64 / self.tasks.len() as f64
    }

    /// One sentence, derived from the account, safe to quote without the table.
    pub fn headline(&self) -> String {
        let captured = match self.account.captured_gain_fraction() {
            Some(fraction) => format!("{:.1}% of the achievable gain", fraction * 100.0),
            None => "no achievable gain to capture".to_string(),
        };
        match self.verdict {
            RoutingVerdict::RouterLosesToFixedDefault => format!(
                "Routing was WORSE than the fixed default `{}` ({:.4} vs {:.4} mean utility over \
                 {} held-out tasks); the oracle bound was {:.4}. Do not ship this router.",
                architecture_label(&self.account.fixed_default),
                self.account.router.mean_utility,
                self.account.fixed_default.mean_utility,
                self.tasks.len(),
                self.account.oracle.mean_utility
            ),
            RoutingVerdict::NoAchievableGain => format!(
                "The fixed default `{}` already matched the oracle retrospective selector on all \
                 {} held-out tasks, so there was {captured}; the router neither helped nor hurt. \
                 This panel cannot demonstrate routing.",
                architecture_label(&self.account.fixed_default),
                self.tasks.len()
            ),
            RoutingVerdict::RouterMatchesFixedDefault => format!(
                "Routing did not beat the fixed default `{}` on {} held-out tasks: identical mean \
                 utility {:.4}, while the oracle bound was {:.4}. The router captured {captured}.",
                architecture_label(&self.account.fixed_default),
                self.tasks.len(),
                self.account.router.mean_utility,
                self.account.oracle.mean_utility
            ),
            RoutingVerdict::RouterBeatsFixedDefault => format!(
                "Routing beat the fixed default `{}` on {} held-out tasks ({:.4} vs {:.4} mean \
                 utility) and captured {captured} available against the oracle bound {:.4}; it \
                 won {} tasks, lost {} and abstained on {:.0}%.",
                architecture_label(&self.account.fixed_default),
                self.tasks.len(),
                self.account.router.mean_utility,
                self.account.fixed_default.mean_utility,
                self.account.oracle.mean_utility,
                self.tasks_won,
                self.tasks_lost,
                self.abstention_rate * 100.0
            ),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "gate": "6",
            "held_out_tasks": self.tasks.len(),
            "verdict": self.verdict.as_str(),
            "headline": self.headline(),
            "comparators": self.account.all().iter().map(|outcome| json!({
                "name": outcome.name(),
                "architecture": outcome.fixed_architecture().map(Architecture::label),
                "sees_the_answer": outcome.comparator.sees_the_answer(),
                "mean_utility": outcome.mean_utility,
                "mean_regret_against_oracle": outcome.mean_regret,
                "admissible_rate": outcome.admissible_rate,
                "mean_cost_fraction": outcome.mean_cost_fraction,
            })).collect::<Vec<_>>(),
            "achievable_gain": self.account.achievable_gain(),
            "captured_gain": self.account.captured_gain(),
            "captured_gain_fraction": self.account.captured_gain_fraction(),
            "residual_regret": self.account.residual_regret(),
            "win_rate_against_fixed_default": self.win_rate(),
            "tasks_won": self.tasks_won,
            "tasks_lost": self.tasks_lost,
            "tasks_tied": self.tasks_tied,
            "abstention_rate": self.abstention_rate,
            "oracle_agreement_rate_among_routed": self.oracle_agreement_rate,
            "expected_calibration_error": self.calibration.expected_calibration_error(),
            "caveats": self.caveats,
            "tasks": self.tasks.iter().map(|row| json!({
                "task_id": row.task_id,
                "regime": row.regime.to_string(),
                "facts": row.facts,
                "abstained": row.abstained,
                "confidence": row.confidence,
                "router": row.router.architecture.label(),
                "fixed_default": row.fixed_default.architecture.label(),
                "most_expensive_default": row.most_expensive_default.architecture.label(),
                "oracle": row.oracle.architecture.label(),
                "router_utility": row.router.utility,
                "oracle_utility": row.oracle.utility,
                "router_regret": row.router_regret(),
                "matched_oracle": row.matched_oracle(),
            })).collect::<Vec<_>>(),
        })
    }

    pub fn to_markdown(&self) -> String {
        use std::fmt::Write as _;
        let mut text = String::new();

        let _ = writeln!(
            text,
            "# Gate 6 — evaluation-conditioned inference routing\n\n{} held-out tasks. The \
             evidence used to route each task never contains that task; the holdout note at the \
             end says how much more than that was withheld.\n",
            self.tasks.len()
        );
        let _ = writeln!(text, "**{}**\n", self.headline());

        let _ = writeln!(
            text,
            "| Comparator | Architecture | Sees answer | Mean utility | Regret vs oracle | \
             Admissible | Mean context cost |"
        );
        let _ = writeln!(text, "|---|---|:-:|---:|---:|---:|---:|");
        for outcome in self.account.all() {
            let _ = writeln!(
                text,
                "| {} | {} | {} | {:.4} | {:.4} | {:.0}% | {:.2}% |",
                outcome.name(),
                outcome
                    .fixed_architecture()
                    .map(Architecture::label)
                    .unwrap_or_else(|| "per-task".to_string()),
                if outcome.comparator.sees_the_answer() {
                    "**yes**"
                } else {
                    "no"
                },
                outcome.mean_utility,
                outcome.mean_regret,
                outcome.admissible_rate * 100.0,
                outcome.mean_cost_fraction * 100.0
            );
        }

        let _ = writeln!(
            text,
            "\nThe oracle retrospective selector already knows each task's outcome. It is a bound \
             on what any router could achieve, not a competitor: a router that beats the fixed \
             default proves nothing until its number is read against this row."
        );

        let _ = writeln!(text, "\n## Regret accounting\n");
        let _ = writeln!(
            text,
            "- achievable gain over the fixed default: **{:.4}**",
            self.account.achievable_gain()
        );
        let _ = writeln!(
            text,
            "- gain the router actually captured: **{:.4}**",
            self.account.captured_gain()
        );
        match self.account.captured_gain_fraction() {
            Some(fraction) => {
                let _ = writeln!(
                    text,
                    "- **fraction of achievable gain captured: {:.1}%** — versus a raw win rate of \
                     {:.1}% ({} won, {} lost, {} tied). Where these two disagree, the fraction is \
                     the one that means something.",
                    fraction * 100.0,
                    self.win_rate() * 100.0,
                    self.tasks_won,
                    self.tasks_lost,
                    self.tasks_tied
                );
            }
            None => {
                let _ = writeln!(
                    text,
                    "- **fraction of achievable gain captured: undefined** — the fixed default \
                     already matched the oracle bound, so there was no gain to capture and a \
                     percentage here would be meaningless. Raw win rate {:.1}% ({} won, {} lost, \
                     {} tied) is reported only for completeness.",
                    self.win_rate() * 100.0,
                    self.tasks_won,
                    self.tasks_lost,
                    self.tasks_tied
                );
            }
        }
        let _ = writeln!(
            text,
            "- residual regret still on the table: **{:.4}**",
            self.account.residual_regret()
        );

        let _ = writeln!(text, "\n## Abstention and confidence\n");
        let _ = writeln!(
            text,
            "- abstained to the safe default on {:.1}% of tasks",
            self.abstention_rate * 100.0
        );
        match self.oracle_agreement_rate {
            Some(rate) => {
                let _ = writeln!(
                    text,
                    "- among routed decisions, {:.1}% chose the architecture the oracle would have \
                     chosen",
                    rate * 100.0
                );
            }
            None => {
                let _ = writeln!(
                    text,
                    "- no task was routed; every decision abstained, so there is nothing to \
                     calibrate"
                );
            }
        }
        match self.calibration.expected_calibration_error() {
            Some(error) => {
                let _ = writeln!(
                    text,
                    "- expected calibration error of the confidence score against oracle \
                     agreement: **{error:.3}** over {} routed decisions in {} populated bins",
                    self.calibration.decisions,
                    self.calibration.populated_bins().count()
                );
            }
            None => {
                let _ = writeln!(
                    text,
                    "- calibration error is undefined with zero routed decisions"
                );
            }
        }

        let _ = writeln!(text, "\n## Per-task\n");
        let _ = writeln!(
            text,
            "| Task | Regime | Facts | Router | Conf | Oracle | Router u | Oracle u | Regret |"
        );
        let _ = writeln!(text, "|---|---|---:|---|---:|---|---:|---:|---:|");
        for row in &self.tasks {
            let _ = writeln!(
                text,
                "| {} | {} | {} | {}{} | {:.2} | {} | {:.3} | {:.3} | {:.3} |",
                row.task_id,
                row.regime,
                row.facts,
                row.router.architecture.label(),
                if row.abstained { " *(abstained)*" } else { "" },
                row.confidence,
                row.oracle.architecture.label(),
                row.router.utility,
                row.oracle.utility,
                row.router_regret()
            );
        }

        let _ = writeln!(text, "\n## What this does not measure\n");
        for caveat in &self.caveats {
            let _ = writeln!(text, "- {caveat}");
        }

        text
    }
}

fn architecture_label(outcome: &ComparatorOutcome) -> String {
    outcome
        .fixed_architecture()
        .map(Architecture::label)
        .unwrap_or_else(|| "per-task".to_string())
}

fn validate_task_row(row: &TaskRow) -> Result<(), RoutingError> {
    validate_task_id(&row.task_id)?;
    if !row.confidence.is_finite() || !(0.0..=1.0).contains(&row.confidence) {
        return Err(RoutingError::InvalidReport {
            detail: format!("task `{}` has invalid confidence", row.task_id),
        });
    }
    if row.abstained != row.reason.is_abstention() {
        return Err(RoutingError::InvalidReport {
            detail: format!("task `{}` has inconsistent abstention state", row.task_id),
        });
    }
    if row.abstained && row.confidence != 0.0 {
        return Err(RoutingError::InvalidReport {
            detail: format!("abstained task `{}` must have zero confidence", row.task_id),
        });
    }

    for (name, pick) in [
        ("fixed_default", &row.fixed_default),
        ("most_expensive_default", &row.most_expensive_default),
        ("router", &row.router),
        ("oracle", &row.oracle),
    ] {
        pick.architecture.verify_label()?;
        if !pick.cost_fraction.is_finite()
            || !(0.0..=1.0).contains(&pick.cost_fraction)
            || !pick.utility.is_finite()
        {
            return Err(RoutingError::InvalidReport {
                detail: format!(
                    "task `{}` has invalid {name} comparator metrics",
                    row.task_id
                ),
            });
        }
        let expected_utility = if pick.admissible {
            1.0 - pick.cost_fraction
        } else {
            INADMISSIBLE_UTILITY
        };
        if (pick.utility - expected_utility).abs() > GAIN_EPSILON {
            return Err(RoutingError::InvalidReport {
                detail: format!("task `{}` has unbound {name} utility", row.task_id),
            });
        }
    }
    let best_comparator_utility = [
        row.fixed_default.utility,
        row.most_expensive_default.utility,
        row.router.utility,
    ]
    .into_iter()
    .fold(f64::NEG_INFINITY, f64::max);
    if row.oracle.utility + GAIN_EPSILON < best_comparator_utility {
        return Err(RoutingError::InvalidReport {
            detail: format!(
                "task `{}` oracle is below a comparator outcome",
                row.task_id
            ),
        });
    }
    Ok(())
}

fn outcome<'a>(
    comparator: Comparator,
    picks: impl Iterator<Item = &'a ComparatorPick>,
    oracle_utilities: &[f64],
) -> ComparatorOutcome {
    let picks: Vec<&ComparatorPick> = picks.collect();
    let count = picks.len().max(1) as f64;
    let mean_utility = picks.iter().map(|pick| pick.utility).sum::<f64>() / count;
    let mean_regret = picks
        .iter()
        .zip(oracle_utilities.iter())
        .map(|(pick, oracle)| oracle - pick.utility)
        .sum::<f64>()
        / count;

    ComparatorOutcome {
        comparator,
        tasks: picks.len(),
        mean_utility,
        mean_regret,
        admissible_rate: picks.iter().filter(|pick| pick.admissible).count() as f64 / count,
        mean_cost_fraction: picks.iter().map(|pick| pick.cost_fraction).sum::<f64>() / count,
    }
}

fn default_caveats() -> Vec<String> {
    vec![
        "Routes between context architectures only. No model is selected, no LLM is called, and \
         nothing is trained; blueprint 09.08's contextual bandits and 09.11's online-learning \
         policy are not implemented."
            .to_string(),
        "Utility is admissibility first, context cost second. Latency, token cost, provider risk \
         and data-policy constraints are named in blueprint 09.08 and are not measured here."
            .to_string(),
        "No confidence intervals and no significance tests. Differences below 1e-9 are treated as \
         ties for floating-point reasons only."
            .to_string(),
        "The oracle bound is retrospective over the observed panel. It bounds routing among these \
         approved architectures on these tasks, and says nothing about architectures not entered."
            .to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::task_row;

    #[test]
    fn a_report_over_zero_tasks_is_refused_rather_than_summarised() {
        let error = RoutingReport::assemble(
            Vec::new(),
            Architecture::FiberCompiled,
            Architecture::FullContext,
            5,
        )
        .unwrap_err();
        assert_eq!(error, RoutingError::NoTasks);
    }

    fn report(rows: Vec<TaskRow>) -> RoutingReport {
        RoutingReport::assemble(
            rows,
            Architecture::FiberCompiled,
            Architecture::FullContext,
            5,
        )
        .unwrap()
    }

    #[test]
    fn the_markdown_always_names_all_four_comparators() {
        let markdown = report(vec![task_row("t1", 0.5, 0.0, 0.9, 0.9)]).to_markdown();
        for name in [
            "fixed-default",
            "most-expensive-default",
            "evidence-router",
            "oracle-retrospective",
        ] {
            assert!(markdown.contains(name), "report omitted `{name}`");
        }
        assert!(markdown.contains("bound on what any router could achieve"));
    }

    #[test]
    fn a_router_that_loses_to_the_fixed_default_says_so_plainly() {
        let losing = report(vec![
            task_row("t1", 0.9, 0.0, 0.4, 0.95),
            task_row("t2", 0.9, 0.0, 0.4, 0.95),
        ]);
        assert_eq!(losing.verdict, RoutingVerdict::RouterLosesToFixedDefault);
        let headline = losing.headline();
        assert!(headline.contains("WORSE"), "headline was `{headline}`");
        assert!(headline.contains("Do not ship this router"));
        assert!(losing.to_markdown().contains("WORSE"));
    }

    #[test]
    fn a_report_with_no_achievable_gain_refuses_to_print_a_percentage() {
        let flat = report(vec![
            task_row("t1", 0.9, 0.0, 0.9, 0.9),
            task_row("t2", 0.9, 0.0, 0.9, 0.9),
        ]);
        assert_eq!(flat.verdict, RoutingVerdict::NoAchievableGain);
        assert_eq!(flat.account.captured_gain_fraction(), None);
        assert!(flat.to_markdown().contains("undefined"));
        assert!(flat.to_json()["captured_gain_fraction"].is_null());
    }

    #[test]
    fn the_captured_fraction_is_printed_next_to_the_win_rate() {
        let flattering = report(vec![
            task_row("t1", 0.50, 0.0, 0.53, 0.80),
            task_row("t2", 0.50, 0.0, 0.53, 0.80),
        ]);
        assert!((flattering.win_rate() - 1.0).abs() < 1e-12);
        let markdown = flattering.to_markdown();
        assert!(markdown.contains("fraction of achievable gain captured: 10.0%"));
        assert!(markdown.contains("raw win rate of 100.0%"));
    }

    #[test]
    fn the_caveats_travel_with_the_artefact() {
        let report = report(vec![task_row("t1", 0.5, 0.0, 0.9, 0.9)]);
        let markdown = report.to_markdown();
        assert!(markdown.contains("No model is selected"));
        assert!(report.to_json()["caveats"].as_array().unwrap().len() >= 4);
    }

    #[test]
    fn a_report_round_trips_through_json() {
        let report = report(vec![task_row("t1", 0.5, 0.0, 0.9, 0.9)]);
        let text = serde_json::to_string(&report).unwrap();
        assert_eq!(
            serde_json::from_str::<RoutingReport>(&text).unwrap(),
            report
        );
    }

    #[test]
    fn report_assembly_rejects_identity_confidence_and_metric_drift() {
        let mut padded = task_row("t1", 0.5, 0.0, 0.9, 0.9);
        padded.task_id = " t1".into();
        assert!(matches!(
            RoutingReport::assemble(
                vec![padded],
                Architecture::FiberCompiled,
                Architecture::FullContext,
                5
            )
            .unwrap_err(),
            RoutingError::InvalidTaskId { .. }
        ));

        let mut invalid_confidence = task_row("t1", 0.5, 0.0, 0.9, 0.9);
        invalid_confidence.confidence = f64::NAN;
        assert!(matches!(
            RoutingReport::assemble(
                vec![invalid_confidence],
                Architecture::FiberCompiled,
                Architecture::FullContext,
                5
            )
            .unwrap_err(),
            RoutingError::InvalidReport { .. }
        ));

        let mut drifted_utility = task_row("t1", 0.5, 0.0, 0.9, 0.9);
        drifted_utility.router.utility = 0.1;
        assert!(matches!(
            RoutingReport::assemble(
                vec![drifted_utility],
                Architecture::FiberCompiled,
                Architecture::FullContext,
                5
            )
            .unwrap_err(),
            RoutingError::InvalidReport { .. }
        ));
    }
}
