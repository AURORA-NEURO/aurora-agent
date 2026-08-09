//! Round-trip checks: is the view still traceable to what it came from?
//!
//! 43.01's third non-negotiable invariant is that "every projection is reversible to stable source
//! handles even when it is not information-lossless". Lossy is fine; unreachable is not. A reader
//! who sees a node must be able to name the object in the compiled region it stands for and go
//! read it.
//!
//! This module turns that into two checks a test or a CI gate can run:
//!
//! - [`obstructions_survive`] — every unresolved obligation and every oracle witness is reachable
//!   from the view. This one holds for all four projections and is additionally enforced at render
//!   time by [`crate::FidelityLedger`]; the check here is the independent confirmation, computed
//!   from the rendered body rather than from what the projection *claimed* to carry.
//! - [`evidence_survives`] — every delivered evidence capsule is reachable. This one legitimately
//!   fails for the timeline, whose subject is events rather than evidence, and the failure is
//!   visible in that view's loss ledger rather than hidden.
//!
//! Note what is *not* checked: that the view can be inverted back into a Decision Section. It
//! cannot, and 43.01 does not ask for that — only that the handles survive.

use crate::error::ProjectionError;
use crate::graph::{GraphBody, GraphProjection};
use crate::hypergraph::{HypergraphBody, HypergraphProjection};
use crate::identity::{conflict_id, obligation_id};
use crate::provenance::ProjectionSource;
use crate::table::{TableBody, TableProjection};
use crate::timeline::{TimelineBody, TimelineProjection};
use crate::view::{ProjectedBody, Projection, ProjectionKind, View};
use bioprism_section::DecisionSection;
use bioprism_world::CausalEvent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Which expected handles a view exposes and which it lost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleCoverage {
    pub recovered: Vec<String>,
    pub missing: Vec<String>,
}

impl HandleCoverage {
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }

    fn of(expected: &BTreeSet<String>, exposed: &BTreeSet<String>) -> Self {
        let (recovered, missing): (Vec<String>, Vec<String>) = expected
            .iter()
            .cloned()
            .partition(|handle| exposed.contains(handle));
        HandleCoverage { recovered, missing }
    }
}

/// Handles of every obstruction in the section: obligations and oracle witnesses.
pub fn obstruction_handles(section: &DecisionSection) -> BTreeSet<String> {
    let mut handles: BTreeSet<String> = section
        .unresolved_obligations
        .iter()
        .enumerate()
        .map(|(index, obligation)| obligation_id(index, obligation))
        .collect();
    handles.extend(
        section
            .oracle
            .witnesses
            .iter()
            .enumerate()
            .map(|(index, witness)| conflict_id(index, witness)),
    );
    handles
}

/// Handles of every evidence capsule the section delivered.
pub fn evidence_handles(section: &DecisionSection) -> BTreeSet<String> {
    section
        .selected_evidence
        .iter()
        .map(|capsule| capsule.id.clone())
        .collect()
}

/// Confirms every obstruction is reachable from the rendered body.
pub fn obstructions_survive<B: ProjectedBody>(
    section: &DecisionSection,
    view: &View<B>,
) -> HandleCoverage {
    HandleCoverage::of(&obstruction_handles(section), &view.stable_handles())
}

/// Confirms every delivered evidence capsule is reachable from the rendered body.
pub fn evidence_survives<B: ProjectedBody>(
    section: &DecisionSection,
    view: &View<B>,
) -> HandleCoverage {
    HandleCoverage::of(&evidence_handles(section), &view.stable_handles())
}

/// All four projections of one compiled region, under one bound provenance.
///
/// 43.01 lists graph, hypergraph, timeline and table together because they are alternative
/// readings of the same object, not a pipeline. Producing them from one [`ProjectionSource`] is
/// what makes them comparable: any disagreement between them is a bug in a projection, never a
/// difference in what was compiled.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionBundle {
    pub graph: View<GraphBody>,
    pub hypergraph: View<HypergraphBody>,
    pub timeline: View<TimelineBody>,
    pub table: View<TableBody>,
}

/// Projects a region four ways.
///
/// `events` comes from the world rather than the section, because 43.25 does not put the causal
/// event structure in a Decision Section. Pass an empty slice when no event structure is at hand;
/// the timeline is then empty and says so, rather than inventing one from evidence timestamps.
pub fn project_all(
    section: &DecisionSection,
    events: &[CausalEvent],
    source: ProjectionSource,
) -> Result<ProjectionBundle, ProjectionError> {
    Ok(ProjectionBundle {
        graph: GraphProjection::new().project(section, source.clone())?,
        hypergraph: HypergraphProjection::new().project(section, source.clone())?,
        timeline: TimelineProjection::new(events).project(section, source.clone())?,
        table: TableProjection::new().project(section, source)?,
    })
}

impl ProjectionBundle {
    /// Whether every projection kept every obstruction reachable.
    pub fn obstructions_survive_everywhere(&self, section: &DecisionSection) -> bool {
        obstructions_survive(section, &self.graph).is_complete()
            && obstructions_survive(section, &self.hypergraph).is_complete()
            && obstructions_survive(section, &self.timeline).is_complete()
            && obstructions_survive(section, &self.table).is_complete()
    }

    /// The four loss ledgers, so a caller can show a reader what each view gave up.
    pub fn fidelity_summary(&self) -> Vec<(ProjectionKind, usize)> {
        vec![
            (self.graph.kind(), self.graph.fidelity().total_dropped()),
            (
                self.hypergraph.kind(),
                self.hypergraph.fidelity().total_dropped(),
            ),
            (self.timeline.kind(), self.timeline.fidelity().total_dropped()),
            (self.table.kind(), self.table.fidelity().total_dropped()),
        ]
    }
}
