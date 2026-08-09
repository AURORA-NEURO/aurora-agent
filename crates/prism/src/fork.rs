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
//!
//! An arm can also fail to produce a comparison at all, and blueprint 40.23 names that case
//! `partial arm failure`. [`Arm`] keeps it apart from the two states it is easily confused with,
//! for the reason 43.40 gives about missing evidence generally: an arm nobody could judge and an
//! arm nobody attempted are both "nobody checked", and neither is an arm that ran and lost.

use crate::cell::{Acceptance, DecisionCell};
use crate::architecture::Architecture;
use bioprism_fiber::{oracle, FiberError, Query};
use bioprism_section::{OracleStatus, OracleVerdict};
use bioprism_world::World;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

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

/// Why an arm that ran produced nothing the cell could judge.
///
/// One variant, because there is exactly one way an arm can fail today and inventing more would be
/// the kind of implied capability this workspace treats as a lie. Strategy selection is infallible
/// by trait signature, and every strategy selects ids out of the same world it was handed, so a
/// selection naming a fact the world does not hold is unrepresentable rather than merely unlikely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "failure", rename_all = "snake_case")]
pub enum ArmFailure {
    /// The oracle refused the evidence this arm selected.
    ///
    /// The refusal was previously turned into an abstention, which the cell then rejected — so a
    /// question the oracle declined to answer was reported as an architecture that answered it
    /// wrongly. `facts_exposed` is retained because the cost of the attempt was still incurred.
    #[error("the oracle refused the {facts_exposed} fact(s) this arm selected: {detail}")]
    OracleRefused { facts_exposed: usize, detail: String },
}

/// Why an arm was never attempted.
///
/// A separate type from [`ArmFailure`] rather than a variant of it, so the two cannot be merged by
/// a caller matching loosely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotAttemptedReason {
    /// An earlier arm in the panel already carries this architecture's name.
    ///
    /// Attribution names arms by name, so a second arm answering to the same one cannot be
    /// attributed. Running it anyway would put one name on both sides of the comparison sentence;
    /// declining to run it and saying so is the only reading that leaves the report true.
    DuplicateArchitectureName,
}

impl NotAttemptedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            NotAttemptedReason::DuplicateArchitectureName => "duplicate_architecture_name",
        }
    }
}

impl fmt::Display for NotAttemptedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotAttemptedReason::DuplicateArchitectureName => f.write_str(
                "an earlier arm already carries this name, so this one could not be attributed",
            ),
        }
    }
}

/// What became of one arm of the fork.
///
/// Three states that must stay three. Only [`Arm::Judged`] carries a measurement, and because the
/// tag is on the state, an arm that produced no verdict serialises with no `passed`, `status` or
/// `facts_exposed` key at all — there is nothing for a downstream renderer to coerce, default or
/// average. This mirrors `bioprism_atlas::CapabilityCell`, where an unmeasured capability has no
/// score field rather than a zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Arm {
    /// The arm ran and the cell judged its outcome.
    Judged(Trial),
    /// The arm ran and produced nothing the cell could judge. Never counted as a failure against
    /// the cell, because the cell was never applied.
    Unjudged {
        architecture: String,
        reason: ArmFailure,
    },
    /// The arm was never attempted. Nobody checked.
    NotAttempted {
        architecture: String,
        reason: NotAttemptedReason,
    },
}

impl Arm {
    pub fn architecture(&self) -> &str {
        match self {
            Arm::Judged(trial) => &trial.architecture,
            Arm::Unjudged { architecture, .. } | Arm::NotAttempted { architecture, .. } => {
                architecture
            }
        }
    }

    /// The measurement, or `None` where there is none.
    ///
    /// There is deliberately no accessor that reports an unjudged or unattempted arm as a failed
    /// one, which is the coercion the three states exist to prevent.
    pub fn trial(&self) -> Option<&Trial> {
        match self {
            Arm::Judged(trial) => Some(trial),
            Arm::Unjudged { .. } | Arm::NotAttempted { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForkResult {
    pub cell_id: String,
    pub decision_point: String,
    /// One entry per architecture in the panel, in panel order.
    pub arms: Vec<Arm>,
    /// Architectures that were judged and satisfied the cell, in panel order.
    pub passing: Vec<String>,
    /// Architectures that were judged and did not satisfy the cell, in panel order.
    pub failing: Vec<String>,
    /// Architectures that ran and could not be judged. Never folded into `failing`.
    pub unjudged: Vec<String>,
    /// Architectures that were never attempted. Never folded into `unjudged` or `failing`.
    pub not_attempted: Vec<String>,
    /// One sentence a human can act on.
    pub attribution: String,
}

impl ForkResult {
    /// The arms that reached a judged outcome, in panel order.
    pub fn trials(&self) -> impl Iterator<Item = &Trial> {
        self.arms.iter().filter_map(Arm::trial)
    }

    pub fn cheapest_passing(&self) -> Option<&Trial> {
        self.trials()
            .filter(|trial| trial.passed)
            .min_by_key(|trial| trial.facts_exposed)
    }

    /// Whether this fork establishes that nothing regressed.
    ///
    /// Stronger than an empty `failing` list, and deliberately so. An arm that could not be judged
    /// and an arm that was never attempted are both unchecked, and a fork with no arms at all
    /// checked nothing — under the older reading, an empty panel reported itself regression-free.
    pub fn is_regression_free(&self) -> bool {
        !self.arms.is_empty()
            && self
                .arms
                .iter()
                .all(|arm| arm.trial().is_some_and(|trial| trial.passed))
    }
}

fn verdict_for(world: &World, facts: &BTreeSet<String>) -> Result<OracleVerdict, FiberError> {
    let values: BTreeMap<String, Value> = facts
        .iter()
        .filter_map(|id| world.fact(id))
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect();
    oracle::evaluate(&values)
}

/// Runs every architecture from the identical frozen state.
///
/// Still infallible, and that is a decision rather than an oversight. One world, one query and one
/// cell are taken *once* and shared by every arm, so there is no fork-level precondition left to
/// check — identical state across arms is unrepresentable otherwise, not merely verified. What can
/// go wrong belongs to an individual arm, and a `Result` would answer an arm-level question by
/// discarding the arms that did run, which is the comparison this function exists to produce.
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

    let mut arms: Vec<Arm> = Vec::new();
    let mut named: BTreeSet<&str> = BTreeSet::new();
    for architecture in architectures {
        if !named.insert(architecture.name.as_str()) {
            arms.push(Arm::NotAttempted {
                architecture: architecture.name.clone(),
                reason: NotAttemptedReason::DuplicateArchitectureName,
            });
            continue;
        }

        let strategy = architecture.strategy.build();
        let selection = strategy.select(world, query);
        let verdict = match verdict_for(world, &selection.facts) {
            Ok(verdict) => verdict,
            Err(error) => {
                arms.push(Arm::Unjudged {
                    architecture: architecture.name.clone(),
                    reason: ArmFailure::OracleRefused {
                        facts_exposed: selection.facts.len(),
                        detail: error.to_string(),
                    },
                });
                continue;
            }
        };

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
        arms.push(Arm::Judged(Trial {
            architecture: architecture.name.clone(),
            facts_exposed: selection.facts.len(),
            status: verdict.status.as_str().to_string(),
            witnesses: witnesses.into_iter().collect(),
            protected_recall,
            closure_complete,
            passed: acceptance.passed(),
            acceptance,
        }));
    }

    let names = |wanted: fn(&Arm) -> bool| -> Vec<String> {
        arms.iter()
            .filter(|arm| wanted(arm))
            .map(|arm| arm.architecture().to_string())
            .collect()
    };
    let passing = names(|arm| arm.trial().is_some_and(|trial| trial.passed));
    let failing = names(|arm| arm.trial().is_some_and(|trial| !trial.passed));
    let unjudged = names(|arm| matches!(arm, Arm::Unjudged { .. }));
    let not_attempted = names(|arm| matches!(arm, Arm::NotAttempted { .. }));

    let attribution = attribute(&arms, &passing, &failing, &unjudged, &not_attempted);

    ForkResult {
        cell_id: cell.cell_id.clone(),
        decision_point: cell.decision_point.clone(),
        arms,
        passing,
        failing,
        unjudged,
        not_attempted,
        attribution,
    }
}

/// The sentence a human reads first, and therefore the one that must not overclaim.
///
/// Every clause is qualified by *judged*, and the arms that produced no judgement are appended
/// rather than omitted, so the sentence can never say the panel agreed when part of it was silent.
fn attribute(
    arms: &[Arm],
    passing: &[String],
    failing: &[String],
    unjudged: &[String],
    not_attempted: &[String],
) -> String {
    if arms.is_empty() {
        return "no architectures were run, so this fork establishes nothing".into();
    }

    let mut sentence = if passing.is_empty() && failing.is_empty() {
        "no architecture reached a verdict, so the cell was never exercised".to_string()
    } else if failing.is_empty() {
        let cheapest = arms
            .iter()
            .filter_map(Arm::trial)
            .filter(|trial| trial.passed)
            .min_by_key(|trial| trial.facts_exposed);
        match cheapest {
            Some(trial) => format!(
                "every architecture that reached a verdict satisfied the cell; {} did so on the \
                 least context ({} facts)",
                trial.architecture, trial.facts_exposed
            ),
            None => "every architecture that reached a verdict satisfied the cell".into(),
        }
    } else if passing.is_empty() {
        format!(
            "no architecture satisfied the cell; all {} judged arms failed, so the cell is \
             unreachable rather than discriminating",
            failing.len()
        )
    } else {
        let closure_failures: Vec<&Trial> = arms
            .iter()
            .filter_map(Arm::trial)
            .filter(|trial| matches!(trial.acceptance, Acceptance::ClosureIncomplete))
            .collect();

        let mut discriminating = format!(
            "context policy explains the difference: {} satisfied the cell, {} did not, on \
             identical world, query and oracle",
            passing.join(" and "),
            failing.join(" and ")
        );
        if !closure_failures.is_empty() {
            discriminating.push_str(&format!(
                ". {} reached an acceptable verdict from an incomplete protected closure, which \
                 the cell rejects",
                closure_failures
                    .iter()
                    .map(|trial| trial.architecture.as_str())
                    .collect::<Vec<_>>()
                    .join(" and ")
            ));
        }
        discriminating
    };

    if !unjudged.is_empty() {
        sentence.push_str(&format!(
            ". {} produced no verdict at all and is counted as neither",
            unjudged.join(" and ")
        ));
    }
    if !not_attempted.is_empty() {
        sentence.push_str(&format!(
            ". {} was never attempted, so nothing here is evidence about it",
            not_attempted.join(" and ")
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
    // An arm with no verdict gets no numbers, not zeroes: a dash cannot be read as a measurement.
    for arm in &result.arms {
        let _ = match arm {
            Arm::Judged(trial) => writeln!(
                text,
                "| {} | {} | {} | {:.0}% | {} |",
                trial.architecture,
                trial.facts_exposed,
                trial.status,
                trial.protected_recall * 100.0,
                if trial.passed { "pass" } else { "**fail**" }
            ),
            Arm::Unjudged { architecture, .. } => writeln!(
                text,
                "| {architecture} | — | — | — | **unjudged** |"
            ),
            Arm::NotAttempted { architecture, .. } => writeln!(
                text,
                "| {architecture} | — | — | — | *not attempted* |"
            ),
        };
    }
    for arm in &result.arms {
        let note = match arm {
            Arm::Judged(trial) if trial.passed => None,
            Arm::Judged(trial) => Some(format!(
                "`{}`: {}",
                trial.architecture,
                trial.acceptance.reason()
            )),
            Arm::Unjudged {
                architecture,
                reason,
            } => Some(format!("`{architecture}`: {reason}")),
            Arm::NotAttempted {
                architecture,
                reason,
            } => Some(format!("`{architecture}`: {reason}")),
        };
        if let Some(note) = note {
            let _ = writeln!(text, "\n- {note}");
        }
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
