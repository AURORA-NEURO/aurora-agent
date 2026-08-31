//! The table projection: the accessible fallback.
//!
//! Blueprint 40.29 makes it an invariant that "every graph has table fallback", and 42.27 requires
//! "table, outline, keyboard, screen-reader, text, and API equivalents for every core graph task".
//! This is the equivalent, expressed as data: a fixed column list and one row per object, which a
//! screen reader, a terminal or a CSV writer can consume without a layout engine.
//!
//! Two ordering rules are load-bearing rather than cosmetic:
//!
//! - Obligations and conflicts come **first**, ahead of evidence. 43.25 orders a Decision Section
//!   so that "conflicts and unresolved obligations appear *before* any narrative rendering", and a
//!   table read top to bottom by a screen reader is a narrative rendering.
//! - Nothing is truncated. 40.29 forbids silently truncating protected invariants, and a fallback
//!   that elides long values to fit a terminal is a fallback that can hide the decisive one.
//!
//! What the table loses: typed, directed edges. A relation becomes text in the `relates_to` cell,
//! which a reader can follow but a machine cannot type-check. That is recorded in the loss ledger,
//! pointing back at the graph projection.

use crate::error::ProjectionError;
use crate::factors::selected_factors;
use crate::fidelity::{DropReason, FidelityLedger};
use crate::identity::{decision_node_id, oracle_node_id, refinement_id};
use crate::markers::{conflicts, obligations};
use crate::view::{ProjectedBody, Projection, ProjectionKind};
use crate::vocabulary::{NodeKind, NodeStatus};
use bioprism_section::DecisionSection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The column header, fixed so a consumer can bind to positions.
pub const COLUMNS: [&str; 6] = ["kind", "id", "label", "status", "relates_to", "detail"];

/// One row. Every cell is text: the fallback's contract is that it needs no renderer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    pub kind: NodeKind,
    pub id: String,
    pub label: String,
    /// Typed status rather than a colour, so the row carries its meaning in text (40.29).
    pub status: NodeStatus,
    /// Related objects, flattened to text. The typed form is in the graph projection.
    pub relates_to: String,
    /// Values, scopes and witness descriptions, in full. Never truncated.
    pub detail: String,
}

impl TableRow {
    /// The row as cells in `COLUMNS` order, for a writer that wants positional output.
    pub fn cells(&self) -> [String; 6] {
        [
            self.kind.as_str().to_string(),
            self.id.clone(),
            self.label.clone(),
            self.status.as_str().to_string(),
            self.relates_to.clone(),
            self.detail.clone(),
        ]
    }
}

/// The rendered table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableBody {
    pub columns: Vec<String>,
    /// A one-line statement of what the table is, for a screen reader that announces captions.
    pub caption: String,
    pub rows: Vec<TableRow>,
    /// Index of the first row that is not an obligation or a conflict. Everything before it is an
    /// obstruction, by construction.
    pub obstruction_rows: usize,
}

impl TableBody {
    pub fn row(&self, id: &str) -> Option<&TableRow> {
        self.rows.iter().find(|row| row.id == id)
    }
}

impl ProjectedBody for TableBody {
    fn stable_handles(&self) -> BTreeSet<String> {
        self.rows.iter().map(|row| row.id.clone()).collect()
    }
}

/// Projects a compiled region as a flat table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableProjection;

impl TableProjection {
    pub const fn new() -> Self {
        TableProjection
    }
}

impl Projection for TableProjection {
    type Body = TableBody;
    const KIND: ProjectionKind = ProjectionKind::Table;

    fn render(
        &self,
        section: &DecisionSection,
        ledger: &mut FidelityLedger,
    ) -> Result<TableBody, ProjectionError> {
        let factors = selected_factors(section)?;
        let mut rows: Vec<TableRow> = Vec::new();

        for marker in obligations(section) {
            rows.push(TableRow {
                kind: NodeKind::Obligation,
                id: marker.id.clone(),
                label: marker.kind.clone(),
                status: NodeStatus::Blocked,
                relates_to: marker.handles.join(", "),
                detail: marker.detail.clone(),
            });
            ledger.carry_obligation(marker.id);
        }

        for marker in conflicts(section) {
            rows.push(TableRow {
                kind: NodeKind::Conflict,
                id: marker.id.clone(),
                label: marker.witness_kind.clone(),
                status: NodeStatus::Contradicted,
                relates_to: section.oracle.oracle_kind.clone(),
                detail: marker.detail.clone(),
            });
            ledger.carry_conflict(marker.id);
        }

        let obstruction_rows = rows.len();

        rows.push(TableRow {
            kind: NodeKind::Decision,
            id: decision_node_id(&section.query_id),
            label: section.goal.clone(),
            status: if section.requires_refinement() {
                NodeStatus::Blocked
            } else {
                NodeStatus::Delivered
            },
            relates_to: section.world_id.clone(),
            detail: format!("decision cut {}", section.decision_time),
        });

        rows.push(TableRow {
            kind: NodeKind::Oracle,
            id: oracle_node_id(&section.oracle.oracle_kind),
            label: section.oracle.oracle_kind.clone(),
            status: if section.oracle.witnesses.is_empty() {
                NodeStatus::Delivered
            } else {
                NodeStatus::Contradicted
            },
            relates_to: decision_node_id(&section.query_id),
            detail: format!(
                "status {}; witnesses [{}]",
                section.oracle.status.as_str(),
                section.oracle.witness_kinds().join(", ")
            ),
        });

        for capsule in &section.selected_evidence {
            rows.push(TableRow {
                kind: NodeKind::Evidence,
                id: capsule.id.clone(),
                label: capsule.provides.clone(),
                status: NodeStatus::Delivered,
                relates_to: capsule.provenance.join(", "),
                detail: format!(
                    "value {}; scope {}; tags [{}]",
                    capsule.value,
                    capsule.scope,
                    capsule.tags.join(", ")
                ),
            });
        }

        for factor in &factors {
            rows.push(TableRow {
                kind: NodeKind::Factor,
                id: factor.id.clone(),
                label: factor.kind.clone(),
                status: NodeStatus::Delivered,
                relates_to: factor.inputs.join(", "),
                detail: format!(
                    "inputs [{}]; outputs [{}]; arity {}{}",
                    factor.inputs.join(", "),
                    factor.outputs.join(", "),
                    factor.arity(),
                    if factor.is_multiway() {
                        "; multiway"
                    } else {
                        ""
                    }
                ),
            });
        }

        for (index, option) in section.refinement_frontier.iter().enumerate() {
            rows.push(TableRow {
                kind: NodeKind::Refinement,
                id: refinement_id(index, &option.action),
                label: option.action.clone(),
                status: NodeStatus::Delivered,
                relates_to: option.facts.join(", "),
                detail: format!("would acquire {} fact(s)", option.facts.len()),
            });
        }

        if !section.selected_evidence.is_empty() || !factors.is_empty() {
            ledger.drop_feature(
                "typed directed edges",
                DropReason::FlattenedToText,
                rows.len(),
                COLUMNS.iter().take(1).map(|c| (*c).to_string()).collect(),
                "graph projection",
            );
        }

        Ok(TableBody {
            columns: COLUMNS.iter().map(|c| (*c).to_string()).collect(),
            caption: format!(
                "Compiled region {} of world {} at cut {}: {} row(s), {} obstruction row(s) first.",
                section.query_id,
                section.world_id,
                section.decision_time,
                rows.len(),
                obstruction_rows
            ),
            obstruction_rows,
            rows,
        })
    }
}
