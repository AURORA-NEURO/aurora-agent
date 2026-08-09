//! Decision segmentation and candidate ranking.
//!
//! Blueprint 06 and Gate 2: "trajectory segmentation, candidate decision ranking". A forty-step run
//! contains a handful of points where the outcome was actually in play; the rest are consequences.
//! Segmentation proposes those points and ranks them, so a reviewer reads five candidates instead
//! of forty steps.
//!
//! Ranking is deliberately transparent arithmetic over recorded structure, not a model. Gate 2
//! requires human approval precisely because an opaque ranker that also published its own cells
//! would be unauditable, and 39.07 asks for a *transparent* value heuristic. Every component of a
//! score is reported alongside it.

use crate::event::{EventKind, Trace};
use serde::{Deserialize, Serialize};

/// Why a step looks like a decision worth testing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateScore {
    /// The step had alternatives recorded.
    pub alternatives: usize,
    /// Evidence became visible at this step that was not visible before.
    pub newly_visible: usize,
    /// How much of the run happened after this point. An early decision explains more.
    pub downstream_steps: usize,
    /// The step is where two runs first diverged.
    pub is_divergence: bool,
    pub total: f64,
}

impl CandidateScore {
    fn compute(
        alternatives: usize,
        newly_visible: usize,
        downstream_steps: usize,
        is_divergence: bool,
        trace_len: usize,
    ) -> Self {
        let reach = if trace_len == 0 {
            0.0
        } else {
            downstream_steps as f64 / trace_len as f64
        };
        let total = (alternatives as f64).min(4.0) * 0.25
            + (newly_visible as f64).min(4.0) * 0.15
            + reach * 0.3
            + if is_divergence { 0.5 } else { 0.0 };

        CandidateScore {
            alternatives,
            newly_visible,
            downstream_steps,
            is_divergence,
            total,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub step: usize,
    pub kind: String,
    pub summary: String,
    pub score: CandidateScore,
}

/// Proposes decision boundaries, best first.
///
/// `divergence_step` is optional: with a passing counterpart to compare against, the step where the
/// runs parted is the strongest signal available. Without one, ranking falls back to structure
/// alone and is correspondingly weaker — which the caller can see from `is_divergence` being false
/// on every candidate.
pub fn segment(trace: &Trace, divergence_step: Option<usize>) -> Vec<Candidate> {
    let mut seen_so_far: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut candidates = Vec::new();
    let total = trace.events.len();

    for (index, event) in trace.events.iter().enumerate() {
        let newly_visible = event
            .visible
            .iter()
            .filter(|name| !seen_so_far.contains(name.as_str()))
            .count();
        for name in &event.visible {
            seen_so_far.insert(name.as_str());
        }

        if !event.kind.is_decision_bearing() {
            continue;
        }

        let alternatives = event
            .payload
            .get("alternatives")
            .and_then(|v| v.as_array())
            .map(Vec::len)
            .unwrap_or(0);

        let summary = event
            .payload
            .get("summary")
            .or_else(|| event.payload.get("tool"))
            .and_then(|v| v.as_str())
            .unwrap_or("<no summary recorded>")
            .to_string();

        candidates.push(Candidate {
            step: event.step,
            kind: event.kind.as_str().to_string(),
            summary,
            score: CandidateScore::compute(
                alternatives,
                newly_visible,
                total.saturating_sub(index + 1),
                divergence_step == Some(event.step),
                total,
            ),
        });
    }

    candidates.sort_by(|a, b| {
        b.score
            .total
            .partial_cmp(&a.score.total)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.step.cmp(&b.step))
    });
    candidates
}

/// Counts how many steps a reviewer avoids reading.
///
/// Reported rather than asserted: on a trace that is all decisions the reduction is zero, and
/// claiming otherwise would be the same inflation the mutation engine's diversity accounting
/// exists to prevent.
pub fn review_reduction(trace: &Trace, candidates: &[Candidate]) -> f64 {
    if trace.events.is_empty() {
        return 0.0;
    }
    1.0 - (candidates.len() as f64 / trace.events.len() as f64)
}

/// Steps that can never host a cell, with the reason.
pub fn excluded(trace: &Trace) -> Vec<(usize, &'static str)> {
    trace
        .events
        .iter()
        .filter(|event| !event.kind.is_decision_bearing())
        .map(|event| {
            let reason = match event.kind {
                EventKind::Observation | EventKind::Result => {
                    "happened to the agent; no alternative was available"
                }
                EventKind::Goal => "states the task rather than a choice within it",
                EventKind::Claim => "reports a belief rather than an action",
                EventKind::Termination => "the run had already ended",
                _ => "not decision-bearing",
            };
            (event.step, reason)
        })
        .collect()
}
