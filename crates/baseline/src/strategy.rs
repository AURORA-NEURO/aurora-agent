//! The context-strategy interface.
//!
//! Blueprint 43.38 requires *equal-engineering* comparison: "No baseline receives less data access
//! or weaker tools." Every strategy therefore sees the same [`World`] and the same [`Query`] and
//! returns a set of fact ids. None of them is handed a filtered world, a smaller budget, or a
//! pre-computed answer.
//!
//! A strategy that returns fewer facts has not necessarily won. What matters is whether its
//! selection still supports the correct decision, which [`crate::compare`] measures by running the
//! deterministic oracle over each selection and comparing verdicts.

use crate::index::PanelIndex;
use bioprism_fiber::Query;
use bioprism_world::World;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct Selection {
    pub facts: BTreeSet<String>,
    /// Anything a reader needs in order to judge whether this run was fair.
    pub notes: Vec<String>,
}

impl Selection {
    pub fn new(facts: BTreeSet<String>) -> Self {
        Selection {
            facts,
            notes: Vec::new(),
        }
    }

    pub fn noting(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

pub trait ContextStrategy {
    fn name(&self) -> String;

    /// One line on how this strategy decides, so a reader can tell a real baseline from a
    /// strawman without reading the implementation.
    fn method(&self) -> String;

    /// The selection, given the intermediates [`crate::compare::compare`] built once for the whole
    /// panel.
    ///
    /// This is the only method an implementor writes, and the reason it is the indexed one rather
    /// than the plainer [`ContextStrategy::select`] is that a [`PanelIndex`] can always be built
    /// from a world and a query, while the reverse — recovering a shared index from a bare pair —
    /// is exactly the rebuilding this trait exists to avoid. A strategy that reads no cell pays
    /// nothing for the index it was handed, because the cells are lazy.
    ///
    /// The pair was briefly defaulted the other way, each in terms of the other: `select_indexed`
    /// fell back to `select`, and every shipped strategy defined `select` as `select_indexed` over
    /// a private index of one. That compiles, and an implementor who wrote neither — or only the
    /// one that already had a default — got infinite recursion at run time instead of a missing
    /// method at compile time. With one required method the cycle has no way to exist.
    fn select_indexed(&self, index: &PanelIndex<'_>) -> Selection;

    /// The same selection for a caller holding only a world and a query.
    ///
    /// Builds a private index of one, which is the pre-sharing behaviour and the thing
    /// `tests/shared_index.rs` compares the shared path against. Not intended to be overridden:
    /// there is only ever one implementation of the selection, so the two entry points cannot
    /// drift apart.
    fn select(&self, world: &World, query: &Query) -> Selection {
        self.select_indexed(&PanelIndex::new(world, query))
    }
}

/// Every fact in the world. The upper bound on recall and the thing to beat on cost.
pub struct FullContext;

impl ContextStrategy for FullContext {
    fn name(&self) -> String {
        "full-context".into()
    }

    fn method(&self) -> String {
        "expose every fact in the world".into()
    }

    fn select_indexed(&self, index: &PanelIndex<'_>) -> Selection {
        Selection::new(
            index
                .world()
                .facts
                .iter()
                .map(|fact| fact.id.as_str().to_string())
                .collect(),
        )
        .noting("upper bound on decisive-evidence recall by construction")
    }
}

/// The FIBER compiler, entered as one competitor among the others.
pub struct FiberCompiled;

impl ContextStrategy for FiberCompiled {
    fn name(&self) -> String {
        "fiber".into()
    }

    fn method(&self) -> String {
        "protected closure, then backward dependency slice, then temporal cut".into()
    }

    fn select_indexed(&self, index: &PanelIndex<'_>) -> Selection {
        match bioprism_fiber::compile(index.world(), index.query()) {
            Ok(out) => Selection::new(out.certificate.selected_facts.into_iter().collect()),
            Err(error) => Selection::new(BTreeSet::new())
                .noting(format!("compile failed, selection is empty: {error}")),
        }
    }
}
