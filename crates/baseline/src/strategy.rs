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

    fn select(&self, world: &World, query: &Query) -> Selection;
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

    fn select(&self, world: &World, _query: &Query) -> Selection {
        Selection::new(
            world
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

    fn select(&self, world: &World, query: &Query) -> Selection {
        match bioprism_fiber::compile(world, query) {
            Ok(out) => Selection::new(out.certificate.selected_facts.into_iter().collect()),
            Err(error) => Selection::new(BTreeSet::new())
                .noting(format!("compile failed, selection is empty: {error}")),
        }
    }
}
