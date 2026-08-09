//! The temporal cut.
//!
//! Blueprint 43.09: model evidence availability as a partially ordered event structure rather
//! than reading every artifact that exists at benchmark completion.
//!
//! A variable is readable at a decision time when either no event governs it — it was always
//! there — or some event that produces it had become *available* by the cut. Availability, not
//! occurrence: a molecular result describing a January specimen but released in June is not
//! readable by a January decision. Conflating the two is precisely the temporal-leakage defect
//! the first vertical slice injects and expects the compiler to catch.
//!
//! The cut is built from the event structure alone, never by enumerating facts. Events describe
//! releases and are few; facts are the corpus. Testing accessibility per variable rather than
//! materialising an accessible-set is what keeps this pass independent of world size.

use bioprism_scope::Timestamp;
use bioprism_world::WorldSource;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalCut {
    pub at: Timestamp,
    /// Variables produced by an event that had been released by the cut.
    released: BTreeSet<String>,
    /// Variables governed by some event, whether or not it has been released.
    event_managed: BTreeSet<String>,
}

impl TemporalCut {
    /// Whether `variable` may be read by a decision taken at this cut.
    ///
    /// A variable no event governs was always available. One that an event governs is readable
    /// only once that event has been released.
    pub fn is_accessible(&self, variable: &str) -> bool {
        !self.event_managed.contains(variable) || self.released.contains(variable)
    }

    /// Variables an event governs that have not yet been released at this cut.
    pub fn withheld(&self) -> impl Iterator<Item = &String> {
        self.event_managed
            .iter()
            .filter(|variable| !self.released.contains(variable.as_str()))
    }

    pub fn event_managed(&self) -> &BTreeSet<String> {
        &self.event_managed
    }
}

pub fn temporal_cut<S: WorldSource + ?Sized>(source: &S, at: Timestamp) -> TemporalCut {
    let mut released = BTreeSet::new();
    let mut event_managed = BTreeSet::new();

    for event in source.events() {
        let available = event.is_available_at(at);
        for variable in &event.produces {
            event_managed.insert(variable.as_str().to_string());
            if available {
                released.insert(variable.as_str().to_string());
            }
        }
    }

    TemporalCut {
        at,
        released,
        event_managed,
    }
}
