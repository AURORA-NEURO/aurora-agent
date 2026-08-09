//! Event and Trace IR.
//!
//! Blueprint 03.01 and 03.02. A trajectory is a sequence of typed events, not a transcript. The
//! distinction is the point of the whole section: a transcript records what was *said*, while an
//! event records what was *done* — which tool ran, with what arguments, against what state, and
//! what came back. Only the latter can be replayed or forked.
//!
//! Events carry a monotonically increasing step index and an optional causal parent. Wall-clock
//! ordering is deliberately not authoritative: concurrent tool calls interleave, and a trajectory
//! that reorders under a different scheduler must still segment identically.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// The task or sub-task the agent was given.
    Goal,
    /// Evidence entered the agent's context.
    Observation,
    /// The agent chose among alternatives. These are the candidate decision boundaries.
    Choice,
    /// A tool was invoked.
    Action,
    /// A tool returned.
    Result,
    /// The agent asserted something it believed.
    Claim,
    /// The run ended.
    Termination,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Goal => "goal",
            EventKind::Observation => "observation",
            EventKind::Choice => "choice",
            EventKind::Action => "action",
            EventKind::Result => "result",
            EventKind::Claim => "claim",
            EventKind::Termination => "termination",
        }
    }

    /// Whether an event of this kind can host a decision boundary.
    ///
    /// A cell must freeze a point where the agent had a genuine alternative. Observations and
    /// results are things that happened *to* the agent; freezing one produces a cell where every
    /// architecture does the same thing, which measures nothing.
    pub fn is_decision_bearing(self) -> bool {
        matches!(self, EventKind::Choice | EventKind::Action)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub step: usize,
    pub kind: EventKind,
    /// What the agent did or received, opaque to this crate.
    pub payload: Value,
    /// The step this event responds to, where the producer recorded one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caused_by: Option<usize>,
    /// Variables the agent could see at this step. Used to compare what two runs knew.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible: Vec<String>,
}

impl Event {
    pub fn new(step: usize, kind: EventKind, payload: Value) -> Self {
        Event {
            step,
            kind,
            payload,
            caused_by: None,
            visible: Vec::new(),
        }
    }

    pub fn caused_by(mut self, step: usize) -> Self {
        self.caused_by = Some(step);
        self
    }

    pub fn seeing(mut self, visible: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.visible = visible.into_iter().map(Into::into).collect();
        self
    }

    /// A digest over the semantic content of the event, excluding its position.
    ///
    /// Divergence detection compares *what happened*, not where it sat in the sequence. Including
    /// the step index would report every event after an insertion as divergent, which is how a
    /// naive diff turns one real difference into fifty.
    pub fn content_digest(&self) -> ContentHash {
        let body = serde_json::json!({
            "kind": self.kind.as_str(),
            "payload": self.payload,
            "visible": self.visible,
        });
        ContentHash::of_value(&body).expect("event payload is finite JSON")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub trace_id: String,
    pub events: Vec<Event>,
    /// Whether the run achieved its goal. Set by the producer, not inferred here.
    pub succeeded: bool,
}

impl Trace {
    pub fn new(trace_id: impl Into<String>, events: Vec<Event>, succeeded: bool) -> Self {
        Trace {
            trace_id: trace_id.into(),
            events,
            succeeded,
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn at(&self, step: usize) -> Option<&Event> {
        self.events.iter().find(|event| event.step == step)
    }

    pub fn decision_bearing(&self) -> impl Iterator<Item = &Event> {
        self.events
            .iter()
            .filter(|event| event.kind.is_decision_bearing())
    }

    pub fn digest(&self) -> ContentHash {
        ContentHash::of_value(&serde_json::to_value(self).expect("trace is serialisable"))
            .expect("trace is finite JSON")
    }
}
