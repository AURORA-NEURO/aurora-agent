//! The three clocks §25 keeps insisting on.
//!
//! Blueprint 25.02 requires `event_time` and `record_time` as separate fields; 25.09 adds
//! `decision_time` and lists "clock consistency" and "temporal leakage tests" under validation.
//! The distinction is the whole of the leakage argument: a model that filters on record time has
//! seen the future of any subject whose result was back-filled, and the two timestamps are
//! indistinguishable once they have been flattened into a column called `time`.
//!
//! So an instant in this crate is never bare. It is a [`Stamped`] — a `Timestamp` that knows which
//! clock produced it — and [`crate::bioql`] refuses to order two instants from different clocks.
//!
//! What is deliberately not here: any notion of clock *skew*, offset or synchronisation. 25.09 asks
//! for "alignment confidence" and this crate carries it as a declared number
//! ([`crate::worldline::AlignmentConfidence`]), because estimating alignment needs the events
//! themselves, and nothing here reads a clock or reconciles two of them.

use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Which clock an instant was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clock {
    /// When the thing happened in the world.
    Event,
    /// When the system learned it. Never earlier than the event it records.
    Record,
    /// When a decision was taken, which bounds what the decision could have seen.
    Decision,
    /// When a value was revealed to a participant, per 25.09's reveal gates.
    Reveal,
}

impl Clock {
    pub const ALL: [Clock; 4] = [Clock::Event, Clock::Record, Clock::Decision, Clock::Reveal];

    pub fn as_str(self) -> &'static str {
        match self {
            Clock::Event => "event_time",
            Clock::Record => "record_time",
            Clock::Decision => "decision_time",
            Clock::Reveal => "reveal_time",
        }
    }

    /// Parses the clock names BioQL's `at` clause accepts.
    pub fn parse(name: &str) -> Option<Clock> {
        match name {
            "event" | "event_time" => Some(Clock::Event),
            "record" | "record_time" => Some(Clock::Record),
            "decision" | "decision_time" => Some(Clock::Decision),
            "reveal" | "reveal_time" => Some(Clock::Reveal),
            _ => None,
        }
    }
}

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An instant that knows which clock it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Stamped {
    pub clock: Clock,
    pub at: Timestamp,
}

impl Stamped {
    pub const fn new(clock: Clock, at: Timestamp) -> Self {
        Stamped { clock, at }
    }

    pub fn event(at: Timestamp) -> Self {
        Stamped::new(Clock::Event, at)
    }

    pub fn record(at: Timestamp) -> Self {
        Stamped::new(Clock::Record, at)
    }

    pub fn decision(at: Timestamp) -> Self {
        Stamped::new(Clock::Decision, at)
    }

    /// Orders two instants only when they share a clock.
    ///
    /// `None` is not "equal" and not "unknown ordering to be filled in later" — it is the refusal.
    /// Callers turn it into whatever typed error their module owns.
    pub fn cmp_same_clock(&self, other: &Stamped) -> Option<std::cmp::Ordering> {
        if self.clock == other.clock {
            Some(self.at.as_nanos_utc().cmp(&other.at.as_nanos_utc()))
        } else {
            None
        }
    }
}
