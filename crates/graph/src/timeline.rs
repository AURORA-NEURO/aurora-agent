//! The timeline projection.
//!
//! Blueprint 43.09 stores evidence availability as a partially ordered event structure with
//! explicit conflict, precisely because "a timestamp list cannot represent causal precedence,
//! concurrent events, alternative branches, or mutually exclusive histories". A timeline is a
//! line. Producing one is therefore a lossy act by definition, and this module's job is to be
//! honest about which loss it committed rather than to pretend the world was linear.
//!
//! The distinction the view exists to preserve is `event_time` versus `availability_time`. 43.09's
//! worked example is exactly this: a molecular result generated before a training run but released
//! afterwards is *not* readable by that run. Collapsing the two fields is the temporal-leakage bug
//! the first vertical slice (43.41) is built to catch, so both are rendered on every entry and the
//! availability verdict against the decision cut is stated per entry rather than implied by
//! position.
//!
//! Two things this projection deliberately does not do:
//!
//! - **It does not filter by relevance.** Every event handed to it is rendered, including events
//!   that produced nothing the section selected. 43.01 defines completeness against query
//!   obligations, and a leakage reviewer's obligation is served precisely by the events that were
//!   *not* selected. Each entry says which of its products the region actually took.
//! - **It does not silently order concurrent events.** Every adjacency in the rendered order
//!   carries a justification: causal precedence, clock only, or arbitrary tie-break. The count of
//!   unjustified adjacencies goes into the loss ledger.

use crate::error::ProjectionError;
use crate::fidelity::{DropReason, FidelityLedger};
use crate::identity::decision_node_id;
use crate::markers::{conflicts, obligations, ConflictMarker, ObligationMarker};
use crate::view::{ProjectedBody, Projection, ProjectionKind};
use bioprism_scope::Timestamp;
use bioprism_section::DecisionSection;
use bioprism_world::CausalEvent;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Which clock the line is drawn against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineAxis {
    /// When things happened.
    EventTime,
    /// When they became legally readable. The default, because a temporal cut reads availability,
    /// not occurrence (43.09).
    AvailabilityTime,
}

impl TimelineAxis {
    pub fn as_str(self) -> &'static str {
        match self {
            TimelineAxis::EventTime => "event_time",
            TimelineAxis::AvailabilityTime => "availability_time",
        }
    }
}

/// Whether an event's products may be read at the decision cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    AvailableAtCut,
    /// Produced, but released after the cut. Filesystem presence is not historical accessibility.
    WithheldUntilAfterCut,
}

/// A clock reading that cannot be true as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockAnomaly {
    /// Availability precedes occurrence: a clock error or a backdated artifact. 43.09 requires it
    /// be reported, never silently corrected.
    AvailableBeforeItHappened,
}

/// Why one entry sits after the one before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderJustification {
    /// First entry; nothing precedes it.
    First,
    /// The previous entry is a causal ancestor. The adjacency is real.
    CausalPrecedence,
    /// The clocks differ but the event structure does not order the pair. The line implies a
    /// precedence the world does not assert.
    ClockOnly,
    /// The clocks agree and there is no causal relation. The order is this view's invention, broken
    /// by event id for determinism.
    Arbitrary,
}

impl OrderJustification {
    /// True when the adjacency was imposed by the rendering rather than by the world.
    pub fn is_imposed(self) -> bool {
        matches!(
            self,
            OrderJustification::ClockOnly | OrderJustification::Arbitrary
        )
    }
}

/// One event on the line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub event_id: String,
    /// When it happened.
    pub event_time: String,
    /// When it became readable. Never merged with `event_time`.
    pub availability_time: String,
    pub availability: Availability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anomaly: Option<ClockAnomaly>,
    pub produces: Vec<String>,
    /// The subset of `produces` that the compiled region actually selected evidence for.
    pub produces_selected: Vec<String>,
    pub causal_parents: Vec<String>,
    /// Events with no causal path in either direction. Rendering them on one line orders them; the
    /// world does not.
    pub concurrent_with: Vec<String>,
    pub order_justification: OrderJustification,
}

/// The rendered timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineBody {
    pub decision_node: String,
    pub axis: TimelineAxis,
    /// The cut the region was compiled at, echoed from the section.
    pub decision_cut: String,
    pub entries: Vec<TimelineEntry>,
    /// Events whose causal parents name events not present in the set handed to this projection.
    pub dangling_parents: Vec<String>,
    /// Events participating in a causal cycle. 43.09 invalidates the affected trace region rather
    /// than picking an order for it.
    pub causal_cycle_members: Vec<String>,
    pub obligations: Vec<ObligationMarker>,
    pub conflicts: Vec<ConflictMarker>,
}

impl TimelineBody {
    pub fn entry(&self, event_id: &str) -> Option<&TimelineEntry> {
        self.entries.iter().find(|entry| entry.event_id == event_id)
    }

    /// Entries whose products a decision at this cut may not read.
    pub fn withheld_at_cut(&self) -> impl Iterator<Item = &TimelineEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.availability == Availability::WithheldUntilAfterCut)
    }

    /// Adjacencies this view invented.
    pub fn imposed_adjacencies(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.order_justification.is_imposed())
            .count()
    }
}

impl ProjectedBody for TimelineBody {
    fn stable_handles(&self) -> BTreeSet<String> {
        let mut handles: BTreeSet<String> = BTreeSet::new();
        handles.insert(self.decision_node.clone());
        for entry in &self.entries {
            handles.insert(entry.event_id.clone());
        }
        for obligation in &self.obligations {
            handles.insert(obligation.id.clone());
            handles.extend(obligation.handles.iter().cloned());
        }
        for conflict in &self.conflicts {
            handles.insert(conflict.id.clone());
        }
        handles
    }
}

/// Projects the causal event structure as an ordered line.
///
/// Borrows the events rather than owning them: they belong to the world, and 43.01's contract is
/// that a projection is temporary while the world is not.
#[derive(Debug, Clone, Copy)]
pub struct TimelineProjection<'a> {
    events: &'a [CausalEvent],
    axis: TimelineAxis,
}

impl<'a> TimelineProjection<'a> {
    /// Draws against availability time, the axis a temporal cut actually reads.
    pub fn new(events: &'a [CausalEvent]) -> Self {
        TimelineProjection {
            events,
            axis: TimelineAxis::AvailabilityTime,
        }
    }

    /// Draws against the named axis. An axis is a rendering choice, not a relevance filter.
    pub fn on_axis(events: &'a [CausalEvent], axis: TimelineAxis) -> Self {
        TimelineProjection { events, axis }
    }

    pub fn axis(&self) -> TimelineAxis {
        self.axis
    }

    fn key(&self, event: &CausalEvent) -> Timestamp {
        match self.axis {
            TimelineAxis::EventTime => event.event_time,
            TimelineAxis::AvailabilityTime => event.availability_time,
        }
    }
}

impl Projection for TimelineProjection<'_> {
    type Body = TimelineBody;
    const KIND: ProjectionKind = ProjectionKind::Timeline;

    fn render(
        &self,
        section: &DecisionSection,
        ledger: &mut FidelityLedger,
    ) -> Result<TimelineBody, ProjectionError> {
        let cut = Timestamp::parse(&section.decision_time).map_err(|error| {
            ProjectionError::UnreadableDecisionTime {
                value: section.decision_time.clone(),
                detail: error.to_string(),
            }
        })?;

        let selected: BTreeSet<&str> = section
            .selected_evidence
            .iter()
            .map(|capsule| capsule.provides.as_str())
            .collect();

        let known: BTreeSet<&str> = self.events.iter().map(|e| e.id.as_str()).collect();
        let ancestry = Ancestry::of(self.events);

        let mut ordered: Vec<&CausalEvent> = self.events.iter().collect();
        ordered.sort_by(|left, right| {
            self.key(left)
                .cmp(&self.key(right))
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });

        let mut dangling_parents: Vec<String> = Vec::new();
        let mut entries: Vec<TimelineEntry> = Vec::with_capacity(ordered.len());

        for (position, event) in ordered.iter().enumerate() {
            let id = event.id.as_str().to_string();
            for parent in &event.causal_parents {
                if !known.contains(parent.as_str()) {
                    dangling_parents.push(format!("{id} -> {parent}"));
                }
            }

            let produces: Vec<String> = event
                .produces
                .iter()
                .map(|variable| variable.as_str().to_string())
                .collect();
            let produces_selected: Vec<String> = produces
                .iter()
                .filter(|variable| selected.contains(variable.as_str()))
                .cloned()
                .collect();

            let order_justification = match position.checked_sub(1).map(|prior| ordered[prior]) {
                None => OrderJustification::First,
                Some(previous) => {
                    if ancestry.precedes(previous.id.as_str(), &id) {
                        OrderJustification::CausalPrecedence
                    } else if self.key(previous) == self.key(event) {
                        OrderJustification::Arbitrary
                    } else {
                        OrderJustification::ClockOnly
                    }
                }
            };

            entries.push(TimelineEntry {
                event_time: event.event_time.to_rfc3339(),
                availability_time: event.availability_time.to_rfc3339(),
                availability: if event.is_available_at(cut) {
                    Availability::AvailableAtCut
                } else {
                    Availability::WithheldUntilAfterCut
                },
                anomaly: event
                    .is_backdated()
                    .then_some(ClockAnomaly::AvailableBeforeItHappened),
                produces,
                produces_selected,
                causal_parents: event
                    .causal_parents
                    .iter()
                    .map(|parent| parent.as_str().to_string())
                    .collect(),
                concurrent_with: ancestry.concurrent_with(&id, &known),
                order_justification,
                event_id: id,
            });
        }

        let imposed: Vec<String> = entries
            .iter()
            .filter(|entry| entry.order_justification.is_imposed())
            .map(|entry| entry.event_id.clone())
            .collect();
        if !imposed.is_empty() {
            ledger.drop_feature(
                "causal partial order",
                DropReason::TotallyOrderedForDisplay,
                imposed.len(),
                imposed.iter().take(3).cloned().collect(),
                "causal_parents and concurrent_with on each entry",
            );
        }

        if !section.selected_evidence.is_empty() {
            ledger.drop_feature(
                "evidence values and scopes",
                DropReason::ValuesElided,
                section.selected_evidence.len(),
                section
                    .selected_evidence
                    .iter()
                    .take(3)
                    .map(|capsule| capsule.id.clone())
                    .collect(),
                "table or graph projection",
            );
        }

        let obligation_markers = obligations(section);
        for marker in &obligation_markers {
            ledger.carry_obligation(marker.id.clone());
        }
        let conflict_markers = conflicts(section);
        for marker in &conflict_markers {
            ledger.carry_conflict(marker.id.clone());
        }

        Ok(TimelineBody {
            decision_node: decision_node_id(&section.query_id),
            axis: self.axis,
            decision_cut: section.decision_time.clone(),
            entries,
            dangling_parents,
            causal_cycle_members: ancestry.cycle_members(),
            obligations: obligation_markers,
            conflicts: conflict_markers,
        })
    }
}

/// Transitive causal ancestry, computed by relaxation to a fixpoint.
///
/// Relaxation rather than a topological walk because 43.09 admits that causal precedence may
/// contain cycles in malformed traces, and a topological walk would either loop or need a special
/// case. A fixpoint terminates regardless, and a node that ends up in its own ancestor set is
/// exactly a cycle member.
struct Ancestry {
    ancestors: BTreeMap<String, BTreeSet<String>>,
}

impl Ancestry {
    fn of(events: &[CausalEvent]) -> Self {
        let mut ancestors: BTreeMap<String, BTreeSet<String>> = events
            .iter()
            .map(|event| {
                (
                    event.id.as_str().to_string(),
                    event
                        .causal_parents
                        .iter()
                        .map(|parent| parent.as_str().to_string())
                        .collect(),
                )
            })
            .collect();

        let ids: Vec<String> = ancestors.keys().cloned().collect();
        let mut changed = true;
        while changed {
            changed = false;
            for id in &ids {
                let current = ancestors.get(id).cloned().unwrap_or_default();
                let mut grown = current.clone();
                for parent in &current {
                    if let Some(inherited) = ancestors.get(parent) {
                        grown.extend(inherited.iter().cloned());
                    }
                }
                if grown.len() != current.len() {
                    ancestors.insert(id.clone(), grown);
                    changed = true;
                }
            }
        }

        Ancestry { ancestors }
    }

    fn precedes(&self, earlier: &str, later: &str) -> bool {
        self.ancestors
            .get(later)
            .is_some_and(|set| set.contains(earlier))
    }

    /// Events with no causal path in either direction, restricted to the events actually rendered.
    fn concurrent_with(&self, id: &str, known: &BTreeSet<&str>) -> Vec<String> {
        known
            .iter()
            .filter(|other| **other != id)
            .filter(|other| !self.precedes(other, id) && !self.precedes(id, other))
            .map(|other| (*other).to_string())
            .collect()
    }

    fn cycle_members(&self) -> Vec<String> {
        self.ancestors
            .iter()
            .filter(|(id, set)| set.contains(*id))
            .map(|(id, _)| id.clone())
            .collect()
    }
}
