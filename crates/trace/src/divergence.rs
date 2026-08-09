//! First-divergence localization.
//!
//! Blueprint 06 and critical-path Gate 2. Given a failing run and a passing one, find the earliest
//! step where they stopped agreeing. That step, not the point where the failure became *visible*,
//! is where a Decision Cell belongs — the executive summary's complaint about end-to-end evaluation
//! is precisely that a 40-step run tells you something went wrong and not where.
//!
//! Comparison is over [`crate::event::Event::content_digest`], which excludes the step index. An
//! inserted event shifts every later index, and a positional diff would then report the whole
//! remainder as divergent. Aligning on content means one insertion reports as one insertion.

use crate::event::{Event, Trace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Divergence {
    /// The two runs never differed. Either they are the same run, or the difference lies outside
    /// what the trace records — which is a finding about the trace, not about the runs.
    Identical,
    /// One run is a strict prefix of the other: it stopped early.
    EarlyTermination {
        at_step: usize,
        shorter: String,
        longer_continued_for: usize,
    },
    /// The runs did different things at the same point.
    Diverged {
        /// Step in the failing trace where they first differ.
        failing_step: usize,
        /// Step in the passing trace where they first differ.
        passing_step: usize,
        /// How many events matched before this point.
        common_prefix: usize,
        failing_did: String,
        passing_did: String,
        /// Evidence the passing run could see and the failing run could not.
        visibility_gap: Vec<String>,
    },
}

impl Divergence {
    pub fn is_localised(&self) -> bool {
        matches!(
            self,
            Divergence::Diverged { .. } | Divergence::EarlyTermination { .. }
        )
    }

    /// The step in the failing trace a cell should freeze, when there is one.
    pub fn failing_step(&self) -> Option<usize> {
        match self {
            Divergence::Diverged { failing_step, .. } => Some(*failing_step),
            Divergence::EarlyTermination { at_step, .. } => Some(*at_step),
            Divergence::Identical => None,
        }
    }
}

fn describe(event: &Event) -> String {
    let detail = event
        .payload
        .get("tool")
        .or_else(|| event.payload.get("action"))
        .or_else(|| event.payload.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or("<opaque payload>");
    format!("{} {}", event.kind.as_str(), detail)
}

/// Finds the earliest point at which two traces stop agreeing.
pub fn first_divergence(failing: &Trace, passing: &Trace) -> Divergence {
    let mut index = 0usize;
    while index < failing.events.len() && index < passing.events.len() {
        let left = &failing.events[index];
        let right = &passing.events[index];
        if left.content_digest() != right.content_digest() {
            let visible_to_passing: std::collections::BTreeSet<&str> =
                right.visible.iter().map(String::as_str).collect();
            let visible_to_failing: std::collections::BTreeSet<&str> =
                left.visible.iter().map(String::as_str).collect();

            return Divergence::Diverged {
                failing_step: left.step,
                passing_step: right.step,
                common_prefix: index,
                failing_did: describe(left),
                passing_did: describe(right),
                visibility_gap: visible_to_passing
                    .difference(&visible_to_failing)
                    .map(|name| (*name).to_string())
                    .collect(),
            };
        }
        index += 1;
    }

    if failing.events.len() == passing.events.len() {
        return Divergence::Identical;
    }

    let (shorter, shorter_id, longer) = if failing.events.len() < passing.events.len() {
        (failing, failing.trace_id.clone(), passing)
    } else {
        (passing, passing.trace_id.clone(), failing)
    };

    Divergence::EarlyTermination {
        at_step: shorter.events.last().map(|e| e.step).unwrap_or(0),
        shorter: shorter_id,
        longer_continued_for: longer.events.len() - shorter.events.len(),
    }
}

/// Whether a divergence is worth turning into a cell.
///
/// Two runs can differ at a step neither of them chose — a tool returned different bytes, say.
/// Freezing that produces a cell where no architecture has a decision to make, so the segmenter's
/// judgement is required before a divergence becomes a candidate.
pub fn is_actionable(divergence: &Divergence, failing: &Trace) -> bool {
    match divergence.failing_step() {
        Some(step) => failing
            .at(step)
            .map(|event| event.kind.is_decision_bearing())
            .unwrap_or(false),
        None => false,
    }
}
