//! First *causal* divergence.
//!
//! Blueprint 06.05. `bioprism_trace::first_divergence` already finds the earliest step at which two
//! runs stopped agreeing, and it already knows the thing most diff-based tools get wrong: a
//! divergence that lands on an observation is not something the agent did. This module does not
//! reimplement any of that. It calls it, and then answers the harder question 06.05 actually poses
//! — the earliest decision where *a different valid choice would likely have changed the outcome*,
//! which is generally not the first textual difference.
//!
//! The two come apart in both directions:
//!
//! - The first difference can be **downstream** of the cause. Two runs can agree for ten steps
//!   because the failing run's bad choice at step 3 had no visible effect until step 13.
//! - The first difference can be **exogenous**. If the runs first differ at an `Observation` or a
//!   `Result`, the environment moved, not the agent. This module refuses to name that a failure the
//!   agent caused, and says so as a verdict rather than nudging the report to a nearby step. That
//!   refusal is the whole reason `bioprism_trace::is_actionable` exists, and inheriting it matters
//!   more than producing an answer.
//!
//!   Note what the refusal is *not* claiming. If the agent had called a different tool, that action
//!   would itself have been the first textual difference; so when the first difference is an
//!   observation, the runs' inputs really did diverge outside the agent's control up to that point.
//!
//! ## Ranking
//!
//! 06.05 asks for ancestors ranked "by necessity, counterfactual effect, reversibility, and
//! explanatory simplicity". Each is computed as transparent arithmetic over recorded structure and
//! reported as a separate field; there is no learned model and no single opaque score. The
//! dependency graph is built backward from the terminal event over two edge kinds: the explicit
//! `caused_by` link a producer recorded, and evidence flow — a step that first made a variable
//! visible is a dependency of every later step that could see it.
//!
//! ## What is deliberately not implemented
//!
//! 06.05's "test top candidates through bounded forks" is absent. Forking is
//! `bioprism_prism::matched_fork`'s job and it needs an executable architecture, which a trace does
//! not carry; running it here would make causal analysis depend on having a runtime. The
//! consequence is honest but real: `counterfactual_effect` here is *observed* difference against a
//! reference trajectory, not measured intervention effect. With no reference trajectory it is zero
//! for every candidate, and the analysis says so through [`CausalAnalysis::reference`].
//!
//! The ranking weights and [`FIRST_CAUSAL_THRESHOLD`] are stated cutoffs, not validated ones. No
//! study of reviewer agreement on real trajectories has been run in this workspace, so the ordering
//! is a reading order for a human, not evidence that the top candidate is the cause.

use crate::error::CausalError;
use bioprism_trace::{first_divergence, is_actionable, Divergence, EventKind, Trace};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Weight of "how much of the failure's causal history flows through this step".
pub const WEIGHT_NECESSITY: f64 = 0.35;
/// Weight of "a reference run did something else here".
pub const WEIGHT_COUNTERFACTUAL: f64 = 0.30;
/// Weight of "a later step could not have undone it".
pub const WEIGHT_IRREVERSIBILITY: f64 = 0.20;
/// Weight of "few steps separate it from the failure".
pub const WEIGHT_SIMPLICITY: f64 = 0.15;

/// Score a candidate must reach before it is named as the first causal divergence.
///
/// A stated cutoff. It is set where a candidate that is merely on the causal path, with no
/// reference disagreement and no irreversibility, does not clear it — because such a step is every
/// step, and naming it would be localization theatre.
pub const FIRST_CAUSAL_THRESHOLD: f64 = 0.50;

/// The components of a candidate's rank, kept separate so a reviewer can disagree with one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalScore {
    /// Fraction of the terminal failure's causal ancestry that becomes unreachable if this step is
    /// removed from the graph. 1.0 means every path to the failure runs through it.
    pub necessity: f64,
    /// Observed disagreement with the reference trajectory at this step. Zero when there is no
    /// reference, which is a statement about the evidence, not about the step.
    pub counterfactual_effect: f64,
    /// How hard the step would be to walk back. Read from a declared `irreversible` flag when the
    /// producer recorded one; otherwise defaulted by event kind, which the blueprint leaves open.
    pub irreversibility: f64,
    /// Inverse graph distance to the failure. A cause two hops away explains more simply than one
    /// twenty hops away.
    pub explanatory_simplicity: f64,
    pub total: f64,
    /// Whether `irreversibility` came from the trace or from this crate's default.
    pub irreversibility_declared: bool,
}

/// A step that could be the cause, with its evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalCandidate {
    pub step: usize,
    pub kind: String,
    pub summary: String,
    pub score: CausalScore,
    /// An earlier divergent decision this one descends from. 06.05 excludes a candidate that is
    /// "fully determined by an earlier unresolved cause": if the agent only reached this step
    /// because of an earlier mistake, the earlier mistake is the finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_unresolved: Option<usize>,
}

/// What the analysis concluded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum CausalVerdict {
    /// The earliest agent-controlled step that clears the threshold and is not downstream of an
    /// earlier unresolved cause.
    FirstCausal { step: usize, score: f64 },
    /// Two or more causes, none an ancestor of the other, each carrying the failure's history.
    /// 06.05: PRISM "should not force one root cause when evidence does not support it".
    Conjunction { steps: Vec<usize> },
    /// The runs first differ at a step the agent did not produce. Named, and refused.
    EnvironmentDivergence {
        at_step: usize,
        kind: String,
        /// The nearest decision upstream of it, offered as context only. It is *not* the cause; it
        /// is where a reviewer would start reading.
        nearest_controlled_ancestor: Option<usize>,
    },
    /// The runs never differed on anything the trace records.
    NoDivergence,
    /// There is a divergence but nothing clears the bar. Reported rather than answered.
    Unlocalizable { reason: String },
}

impl CausalVerdict {
    /// The step a cell should freeze, when the analysis is willing to name one.
    ///
    /// Returns `None` for every verdict that declined to localize, including
    /// [`CausalVerdict::EnvironmentDivergence`]. A caller that wants the environment step must ask
    /// for it by name.
    pub fn cell_step(&self) -> Option<usize> {
        match self {
            CausalVerdict::FirstCausal { step, .. } => Some(*step),
            CausalVerdict::Conjunction { steps } => steps.first().copied(),
            _ => None,
        }
    }

    pub fn localised(&self) -> bool {
        matches!(
            self,
            CausalVerdict::FirstCausal { .. } | CausalVerdict::Conjunction { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalAnalysis {
    pub trace_id: String,
    /// The textual first divergence, exactly as `bioprism_trace` reported it. Carried through
    /// unmodified so a reader can see where the causal answer differs from the diff.
    pub textual: Divergence,
    /// `bioprism_trace::is_actionable` on that divergence.
    pub textual_is_actionable: bool,
    /// Trace id of the reference run, when one was supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    pub terminal_step: usize,
    /// Steps in the terminal failure's causal ancestry, earliest first.
    pub ancestry: Vec<usize>,
    /// Decision-bearing ancestors, best first. Only these can host a cell.
    pub candidates: Vec<CausalCandidate>,
    pub verdict: CausalVerdict,
}

impl CausalAnalysis {
    pub fn first_causal_step(&self) -> Option<usize> {
        self.verdict.cell_step()
    }

    /// Whether the analysis declined to blame the agent.
    pub fn refuses_to_localise(&self) -> bool {
        !self.verdict.localised()
    }

    /// Confirms a step may host a cell before anything is built on it.
    ///
    /// The one guard the rest of the compiler routes through: a step that happened *to* the agent
    /// cannot be a decision, and asking for it returns [`CausalError::NotAgentControlled`] rather
    /// than the nearest plausible substitute.
    pub fn localise_to(&self, trace: &Trace, step: usize) -> Result<usize, CausalError> {
        match trace.at(step) {
            None => Err(CausalError::NoDecisionBearingStep),
            Some(event) if !event.kind.is_decision_bearing() => {
                Err(CausalError::NotAgentControlled {
                    step,
                    kind: event.kind.as_str(),
                })
            }
            Some(_) => Ok(step),
        }
    }
}

fn describe(trace: &Trace, step: usize) -> String {
    trace
        .at(step)
        .and_then(|event| {
            event
                .payload
                .get("summary")
                .or_else(|| event.payload.get("tool"))
                .or_else(|| event.payload.get("action"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("<no summary recorded>")
        .to_string()
}

/// Backward dependency edges: step -> the steps it depends on.
///
/// Two edge kinds, both recorded rather than inferred. `caused_by` is the producer's own causal
/// claim. Evidence flow adds the step at which each variable the event could see first became
/// visible: an agent that acted on a fact depends on whatever put the fact in front of it.
fn dependency_edges(trace: &Trace) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut origin: BTreeMap<&str, usize> = BTreeMap::new();
    for event in &trace.events {
        for name in &event.visible {
            origin.entry(name.as_str()).or_insert(event.step);
        }
    }

    let mut edges: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for event in &trace.events {
        let entry = edges.entry(event.step).or_default();
        if let Some(parent) = event.caused_by {
            if parent < event.step {
                entry.insert(parent);
            }
        }
        for name in &event.visible {
            if let Some(&first) = origin.get(name.as_str()) {
                if first < event.step {
                    entry.insert(first);
                }
            }
        }
    }
    edges
}

fn ancestors_of(
    edges: &BTreeMap<usize, BTreeSet<usize>>,
    target: usize,
    excluding: Option<usize>,
) -> BTreeSet<usize> {
    let mut reached = BTreeSet::new();
    let mut stack = vec![target];
    while let Some(current) = stack.pop() {
        if Some(current) == excluding || !reached.insert(current) {
            continue;
        }
        for parent in edges.get(&current).into_iter().flatten() {
            stack.push(*parent);
        }
    }
    reached.remove(&target);
    reached
}

fn hops_to(edges: &BTreeMap<usize, BTreeSet<usize>>, from: usize, target: usize) -> Option<usize> {
    let mut frontier = vec![target];
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut distance = 0usize;
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for step in frontier {
            if !seen.insert(step) {
                continue;
            }
            if step == from {
                return Some(distance);
            }
            for parent in edges.get(&step).into_iter().flatten() {
                next.push(*parent);
            }
        }
        distance += 1;
        frontier = next;
    }
    None
}

fn irreversibility(trace: &Trace, step: usize) -> (f64, bool) {
    let event = match trace.at(step) {
        Some(event) => event,
        None => return (0.0, false),
    };
    if let Some(declared) = event.payload.get("irreversible").and_then(|v| v.as_bool()) {
        return (if declared { 1.0 } else { 0.0 }, true);
    }
    let defaulted = match event.kind {
        EventKind::Action => 0.5,
        EventKind::Choice => 0.25,
        _ => 0.0,
    };
    (defaulted, false)
}

fn counterfactual_effect(failing: &Trace, reference: Option<&Trace>, step: usize) -> f64 {
    let reference = match reference {
        Some(trace) => trace,
        None => return 0.0,
    };
    let ours = match failing.at(step) {
        Some(event) => event,
        None => return 0.0,
    };
    match reference.at(step) {
        None => 0.5,
        Some(theirs) if theirs.content_digest() != ours.content_digest() => 1.0,
        Some(_) => 0.0,
    }
}

/// Finds the earliest decision that plausibly caused the terminal failure.
///
/// `reference` is a run that did better — a sibling, a peer, or a known-good replay. Without one,
/// `counterfactual_effect` is zero throughout and the analysis is correspondingly weaker; that is
/// visible in the output rather than hidden in the total.
pub fn analyse(failing: &Trace, reference: Option<&Trace>) -> Result<CausalAnalysis, CausalError> {
    if failing.events.is_empty() {
        return Err(CausalError::NoTerminalFailure);
    }
    if failing.decision_bearing().count() == 0 {
        return Err(CausalError::NoDecisionBearingStep);
    }

    let terminal_step = failing
        .events
        .last()
        .map(|event| event.step)
        .ok_or(CausalError::NoTerminalFailure)?;

    let (textual, textual_is_actionable) = match reference {
        Some(reference) => {
            let divergence = first_divergence(failing, reference);
            let actionable = is_actionable(&divergence, failing);
            (divergence, actionable)
        }
        None => (Divergence::Identical, false),
    };

    let edges = dependency_edges(failing);
    let mut ancestry: Vec<usize> = ancestors_of(&edges, terminal_step, None)
        .into_iter()
        .collect();
    ancestry.sort_unstable();
    let full_ancestry = ancestry.len();

    let mut candidates: Vec<CausalCandidate> = Vec::new();
    for &step in &ancestry {
        let event = match failing.at(step) {
            Some(event) => event,
            None => continue,
        };
        if !event.kind.is_decision_bearing() {
            continue;
        }

        let without = ancestors_of(&edges, terminal_step, Some(step)).len();
        let necessity = if full_ancestry == 0 {
            0.0
        } else {
            1.0 - (without as f64 / full_ancestry as f64)
        };
        let effect = counterfactual_effect(failing, reference, step);
        let (irreversibility, declared) = irreversibility(failing, step);
        let simplicity = match hops_to(&edges, step, terminal_step) {
            Some(hops) => 1.0 / (1.0 + hops as f64),
            None => 0.0,
        };
        let total = necessity * WEIGHT_NECESSITY
            + effect * WEIGHT_COUNTERFACTUAL
            + irreversibility * WEIGHT_IRREVERSIBILITY
            + simplicity * WEIGHT_SIMPLICITY;

        candidates.push(CausalCandidate {
            step,
            kind: event.kind.as_str().to_string(),
            summary: describe(failing, step),
            score: CausalScore {
                necessity,
                counterfactual_effect: effect,
                irreversibility,
                explanatory_simplicity: simplicity,
                total,
                irreversibility_declared: declared,
            },
            upstream_unresolved: None,
        });
    }

    let divergent: BTreeSet<usize> = candidates
        .iter()
        .filter(|candidate| candidate.score.counterfactual_effect > 0.0)
        .map(|candidate| candidate.step)
        .collect();
    for candidate in &mut candidates {
        let reachable = ancestors_of(&edges, candidate.step, None);
        candidate.upstream_unresolved = divergent
            .iter()
            .filter(|earlier| **earlier != candidate.step && reachable.contains(*earlier))
            .min()
            .copied();
    }

    let verdict = decide(
        failing,
        &textual,
        textual_is_actionable,
        reference.is_some(),
        &candidates,
        &edges,
    );

    candidates.sort_by(|a, b| {
        b.score
            .total
            .partial_cmp(&a.score.total)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.step.cmp(&b.step))
    });

    Ok(CausalAnalysis {
        trace_id: failing.trace_id.clone(),
        textual,
        textual_is_actionable,
        reference: reference.map(|trace| trace.trace_id.clone()),
        terminal_step,
        ancestry,
        candidates,
        verdict,
    })
}

fn decide(
    failing: &Trace,
    textual: &Divergence,
    actionable: bool,
    has_reference: bool,
    candidates: &[CausalCandidate],
    edges: &BTreeMap<usize, BTreeSet<usize>>,
) -> CausalVerdict {
    if !has_reference {
        return CausalVerdict::Unlocalizable {
            reason: "no reference trajectory was supplied. A divergence is a relation between two \
                     runs; with one run the ranked candidates are structural suspicion, not \
                     evidence that a different choice would have changed the outcome"
                .to_string(),
        };
    }

    if let Divergence::Identical = textual {
        return CausalVerdict::NoDivergence;
    }

    if !actionable {
        let step = match textual.failing_step() {
            Some(step) => step,
            None => {
                return CausalVerdict::Unlocalizable {
                    reason: "the divergence names no step in the failing trace".to_string(),
                }
            }
        };
        return match failing.at(step) {
            Some(event) => {
                let reachable = ancestors_of(edges, step, None);
                let nearest = failing
                    .events
                    .iter()
                    .filter(|other| {
                        other.kind.is_decision_bearing() && reachable.contains(&other.step)
                    })
                    .map(|other| other.step)
                    .next_back();
                CausalVerdict::EnvironmentDivergence {
                    at_step: step,
                    kind: event.kind.as_str().to_string(),
                    nearest_controlled_ancestor: nearest,
                }
            }
            None => CausalVerdict::Unlocalizable {
                reason: format!(
                    "the reference run diverged at step {step}, which the failing run never reached"
                ),
            },
        };
    }

    let mut eligible: Vec<&CausalCandidate> = candidates
        .iter()
        .filter(|candidate| {
            candidate.score.total >= FIRST_CAUSAL_THRESHOLD
                && candidate.upstream_unresolved.is_none()
        })
        .collect();
    eligible.sort_by_key(|candidate| candidate.step);

    let roots: Vec<usize> = eligible
        .iter()
        .filter(|candidate| {
            let reachable = ancestors_of(edges, candidate.step, None);
            eligible
                .iter()
                .all(|other| other.step == candidate.step || !reachable.contains(&other.step))
        })
        .map(|candidate| candidate.step)
        .collect();
    if roots.len() > 1 {
        return CausalVerdict::Conjunction { steps: roots };
    }

    match eligible.first() {
        Some(candidate) => CausalVerdict::FirstCausal {
            step: candidate.step,
            score: candidate.score.total,
        },
        None => CausalVerdict::Unlocalizable {
            reason: format!(
                "no decision-bearing ancestor of the terminal failure scored at least {FIRST_CAUSAL_THRESHOLD:.2}; \
                 {} candidate(s) were considered",
                candidates.len()
            ),
        },
    }
}
