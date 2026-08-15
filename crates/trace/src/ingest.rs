//! Trajectory ingestion.
//!
//! Blueprint 06 and the MVP cut line: "Native JSONL plus one OpenTelemetry adapter; **loss report
//! included**." Native JSONL and [`crate::otel`] share the same requirement — an importer that
//! silently drops a field it did not understand produces a trace that looks complete and replays
//! wrong.
//!
//! One line per event. A line that cannot be typed is recorded as a loss and skipped, never
//! guessed into an `Observation`, because a mistyped event changes where the segmenter puts a
//! boundary.

use crate::error::TraceError;
use crate::event::{Event, EventKind, Trace};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What the importer could not carry across.
///
/// An empty report asserts "nothing was lost"; it is a claim, not an absence of checking, which is
/// why [`Ingestion`] cannot be built without one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportLoss {
    /// Lines that could not be parsed at all.
    pub unparsed_lines: Vec<usize>,
    /// Lines whose `kind` was absent or unrecognised.
    pub untyped_events: Vec<UntypedEvent>,
    /// Fields present in the source that this importer has no home for.
    pub unmapped_fields: Vec<UnmappedField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UntypedEvent {
    pub line: usize,
    pub found: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnmappedField {
    pub line: usize,
    pub field: String,
}

impl ImportLoss {
    pub fn is_lossless(&self) -> bool {
        self.unparsed_lines.is_empty()
            && self.untyped_events.is_empty()
            && self.unmapped_fields.is_empty()
    }

    pub fn dropped_events(&self) -> usize {
        self.unparsed_lines.len() + self.untyped_events.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ingestion {
    trace: Trace,
    loss: ImportLoss,
}

impl Ingestion {
    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    pub fn loss(&self) -> &ImportLoss {
        &self.loss
    }

    pub fn into_parts(self) -> (Trace, ImportLoss) {
        (self.trace, self.loss)
    }

    /// Whether this trace is safe to compile a cell from.
    ///
    /// A trace that dropped events is not: the segmenter would place boundaries in a sequence with
    /// holes in it, and the resulting cell would freeze a state the agent never occupied.
    pub fn is_compilable(&self) -> bool {
        self.loss.dropped_events() == 0 && !self.trace.is_empty()
    }
}

const KNOWN_FIELDS: [&str; 5] = ["step", "kind", "payload", "caused_by", "visible"];

/// Reads newline-delimited JSON events.
pub fn from_jsonl(trace_id: impl Into<String>, text: &str, succeeded: bool) -> Ingestion {
    let mut events = Vec::new();
    let mut loss = ImportLoss::default();

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            loss.unparsed_lines.push(line_number);
            continue;
        };
        let Some(object) = value.as_object() else {
            loss.unparsed_lines.push(line_number);
            continue;
        };

        let kind = match object.get("kind").and_then(Value::as_str) {
            Some("goal") => EventKind::Goal,
            Some("observation") => EventKind::Observation,
            Some("choice") => EventKind::Choice,
            Some("action") => EventKind::Action,
            Some("result") => EventKind::Result,
            Some("claim") => EventKind::Claim,
            Some("termination") => EventKind::Termination,
            other => {
                loss.untyped_events.push(UntypedEvent {
                    line: line_number,
                    found: other.unwrap_or("<absent>").to_string(),
                });
                continue;
            }
        };

        for field in object.keys() {
            if !KNOWN_FIELDS.contains(&field.as_str()) {
                loss.unmapped_fields.push(UnmappedField {
                    line: line_number,
                    field: field.clone(),
                });
            }
        }

        let step = object
            .get("step")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(events.len());

        let mut event = Event::new(
            step,
            kind,
            object.get("payload").cloned().unwrap_or(Value::Null),
        );
        if let Some(parent) = object.get("caused_by").and_then(Value::as_u64) {
            event = event.caused_by(parent as usize);
        }
        if let Some(visible) = object.get("visible").and_then(Value::as_array) {
            event = event.seeing(visible.iter().filter_map(|v| v.as_str().map(str::to_string)));
        }
        events.push(event);
    }

    events.sort_by_key(|event| event.step);
    Ingestion {
        trace: Trace::new(trace_id, events, succeeded),
        loss,
    }
}

/// Checks a trace's structural integrity after import.
///
/// Duplicate or non-monotonic steps mean the producer and this importer disagree about ordering,
/// and every downstream boundary would be placed against the wrong prefix.
pub fn validate(trace: &Trace) -> Result<(), TraceError> {
    let mut previous: Option<usize> = None;
    for event in &trace.events {
        if let Some(last) = previous {
            if event.step == last {
                return Err(TraceError::DuplicateStep { step: event.step });
            }
            if event.step < last {
                return Err(TraceError::NonMonotonicStep {
                    step: event.step,
                    after: last,
                });
            }
        }
        if let Some(parent) = event.caused_by {
            if parent >= event.step {
                return Err(TraceError::CausalParentNotEarlier {
                    step: event.step,
                    parent,
                });
            }
        }
        previous = Some(event.step);
    }
    Ok(())
}
