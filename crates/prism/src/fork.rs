//! Matched counterfactual forks.
//!
//! Every architecture resumes from the *same* frozen state, so the only thing that varies is the
//! context policy. Blueprint 07 puts deterministic evidence at the top of its evaluation ladder,
//! and that is all that is used here: the same deterministic oracle judges every continuation.
//! No model, no judge, no score.
//!
//! Component attribution follows directly. If two architectures differ in outcome and the cell
//! held everything else identical, the difference is attributable to the context policy — which is
//! the claim the executive summary says end-to-end task comparison cannot support.

use crate::cell::{Acceptance, DecisionCell};
use crate::architecture::Architecture;
use bioprism_fiber::{oracle, Query};
use bioprism_section::{OracleStatus, OracleVerdict};
use bioprism_world::World;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trial {
    pub architecture: String,
    pub facts_exposed: usize,
    pub status: String,
    pub witnesses: Vec<String>,
    pub protected_recall: f64,
    pub closure_complete: bool,
    pub acceptance: Acceptance,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForkResult {
    pub cell_id: String,
    pub decision_point: String,
    pub trials: Vec<Trial>,
    /// Architectures that satisfied the cell, cheapest first.
    pub passing: Vec<String>,
    pub failing: Vec<String>,
    /// One sentence a human can act on.
    pub attribution: String,
}

impl ForkResult {
    pub fn cheapest_passing(&self) -> Option<&Trial> {
        self.trials
            .iter()
            .filter(|trial| trial.passed)
            .min_by_key(|trial| trial.facts_exposed)
    }

    pub fn is_regression_free(&self) -> bool {
        self.failing.is_empty()
    }
}

fn verdict_for(world: &World, facts: &BTreeSet<String>) -> OracleVerdict {
    let values: BTreeMap<String, Value> = facts
        .iter()
        .filter_map(|id| world.fact(id))
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect();
    oracle::evaluate(&values).unwrap_or_else(|_| OracleVerdict::abstain(oracle::ORACLE_KIND, vec![]))
}

/// Runs every architecture from the identical frozen state.
pub fn matched_fork(
    cell: &DecisionCell,
    world: &World,
    query: &Query,
    architectures: &[Architecture],
) -> ForkResult {
    let protected_total = world
        .facts
        .iter()
        .filter(|fact| fact.has_any_tag(&query.protected_tags))
        .count();

    let mut trials = Vec::new();
    for architecture in architectures {
        let strategy = architecture.strategy.build();
        let selection = strategy.select(world, query);
        let verdict = verdict_for(world, &selection.facts);

        let witnesses: BTreeSet<String> = verdict
            .witness_kinds()
            .into_iter()
            .map(str::to_string)
            .collect();

        let protected_kept = selection
            .facts
            .iter()
            .filter_map(|id| world.fact(id))
            .filter(|fact| fact.has_any_tag(&query.protected_tags))
            .count();
        let protected_recall = if protected_total == 0 {
            1.0
        } else {
            protected_kept as f64 / protected_total as f64
        };
        let closure_complete = protected_recall >= 1.0;

        let acceptance = cell.accepts(verdict.status, &witnesses, closure_complete);
        trials.push(Trial {
            architecture: architecture.name.clone(),
            facts_exposed: selection.facts.len(),
            status: verdict.status.as_str().to_string(),
            witnesses: witnesses.into_iter().collect(),
            protected_recall,
            closure_complete,
            passed: acceptance.passed(),
            acceptance,
        });
    }

    let passing: Vec<String> = trials
        .iter()
        .filter(|trial| trial.passed)
        .map(|trial| trial.architecture.clone())
        .collect();
    let failing: Vec<String> = trials
        .iter()
        .filter(|trial| !trial.passed)
        .map(|trial| trial.architecture.clone())
        .collect();

    let attribution = attribute(&trials, &passing, &failing);

    ForkResult {
        cell_id: cell.cell_id.clone(),
        decision_point: cell.decision_point.clone(),
        trials,
        passing,
        failing,
        attribution,
    }
}

fn attribute(trials: &[Trial], passing: &[String], failing: &[String]) -> String {
    if trials.is_empty() {
        return "no architectures were run".into();
    }
    if failing.is_empty() {
        let cheapest = trials
            .iter()
            .filter(|trial| trial.passed)
            .min_by_key(|trial| trial.facts_exposed);
        return match cheapest {
            Some(trial) => format!(
                "every architecture satisfied the cell; {} did so on the least context ({} facts)",
                trial.architecture, trial.facts_exposed
            ),
            None => "every architecture satisfied the cell".into(),
        };
    }
    if passing.is_empty() {
        return format!(
            "no architecture satisfied the cell; all {} failed, so the cell is unreachable rather \
             than discriminating",
            failing.len()
        );
    }

    let closure_failures: Vec<&Trial> = trials
        .iter()
        .filter(|trial| matches!(trial.acceptance, Acceptance::ClosureIncomplete))
        .collect();

    let mut sentence = format!(
        "context policy explains the difference: {} satisfied the cell, {} did not, on identical \
         world, query and oracle",
        passing.join(" and "),
        failing.join(" and ")
    );
    if !closure_failures.is_empty() {
        sentence.push_str(&format!(
            ". {} reached an acceptable verdict from an incomplete protected closure, which the \
             cell rejects",
            closure_failures
                .iter()
                .map(|trial| trial.architecture.as_str())
                .collect::<Vec<_>>()
                .join(" and ")
        ));
    }
    sentence
}

/// A compact table for a human reading a CI failure.
pub fn render_table(result: &ForkResult) -> String {
    use std::fmt::Write as _;
    let mut text = String::new();
    let _ = writeln!(
        text,
        "cell {} — {}\n",
        result.cell_id, result.decision_point
    );
    let _ = writeln!(text, "| Architecture | Facts | Verdict | Closure | Cell |");
    let _ = writeln!(text, "|---|---:|---|---:|:-:|");
    for trial in &result.trials {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {:.0}% | {} |",
            trial.architecture,
            trial.facts_exposed,
            trial.status,
            trial.protected_recall * 100.0,
            if trial.passed { "pass" } else { "**fail**" }
        );
    }
    for trial in result.trials.iter().filter(|trial| !trial.passed) {
        let _ = writeln!(text, "\n- `{}`: {}", trial.architecture, trial.acceptance.reason());
    }
    let _ = writeln!(text, "\n{}", result.attribution);
    text
}

pub fn to_json(result: &ForkResult) -> Value {
    json!(result)
}

/// Re-exported so callers can pattern-match without depending on the module path.
pub use crate::cell::Acceptance as TrialAcceptance;

impl From<&Trial> for OracleStatus {
    fn from(trial: &Trial) -> Self {
        match trial.status.as_str() {
            "valid" => OracleStatus::Valid,
            "invalid" => OracleStatus::Invalid,
            _ => OracleStatus::Underdetermined,
        }
    }
}
