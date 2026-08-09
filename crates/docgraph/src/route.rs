//! Task routes (41.05).
//!
//! A route maps "the task I am about to do" onto "the smallest connected part of the corpus that
//! makes it doable". 41.05's objects are route id, intent, must-read nodes, optional nodes,
//! non-omittable nodes, expected outputs and a token budget; its invariants are that "every route
//! resolves", "must-read closure is connected", and "medical/privacy boundaries cannot be
//! optional".
//!
//! # Must-read and non-omittable are not the same list
//!
//! They are kept as two fields even though both end up mandatory, because they fail differently.
//! A must-read node is the *subject* of the task: dropping it means the agent read the wrong
//! thing. A non-omittable node is a boundary that applies whether or not the task is about it —
//! consent scope, claim-kind discipline, the 39.05 protected classes. If they shared a field, an
//! author trimming a route to fit a budget would be free to trim a boundary, and nothing in the
//! type would object. Here [`TaskRoute::optional`] cannot contain a node that
//! [`TaskRoute::non_omittable`] contains; [`TaskRoute::check`] rejects the route rather than
//! silently promoting it, because a route that lists a privacy boundary as optional is a route
//! whose author believed something false.
//!
//! # Budgets are declared, not discovered
//!
//! [`TaskRoute::budget`] is `Option<u32>` of *estimated* tokens ([`crate::tokens`]). `None` means
//! unbounded, which is a legitimate route (an audit reads everything). It does not mean "use a
//! default", because a default budget is a truncation policy nobody wrote down.

use crate::error::DocGraphError;
use crate::registry::{DocGraph, ModuleId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RouteId(String);

impl RouteId {
    pub fn parse(value: impl Into<String>) -> Result<Self, DocGraphError> {
        let value = value.into();
        if value.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(DocGraphError::MalformedModuleId(
                value,
                "route ids are non-empty and whitespace-free",
            ));
        }
        Ok(RouteId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RouteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<RouteId> for String {
    fn from(value: RouteId) -> Self {
        value.0
    }
}

impl TryFrom<String> for RouteId {
    type Error = DocGraphError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        RouteId::parse(value)
    }
}

/// Why a route is malformed. Separate from [`DocGraphError`] because these are authoring
/// mistakes in a route file, not defects in the corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteDefect {
    /// 41.11: "every route node exists".
    MissingModule { module: ModuleId },
    /// 41.05: boundaries "cannot be optional".
    NonOmittableListedAsOptional { module: ModuleId },
    /// A route with no must-read nodes has no subject.
    NoMustRead,
    /// 41.05: "must-read closure is connected". Reported per component, not as a bare flag, so an
    /// author can see which group drifted away.
    MustReadNotConnected { components: Vec<Vec<ModuleId>> },
}

/// One task's documentation slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRoute {
    pub id: RouteId,
    /// What an agent is about to do. Prose; the only field a human reads first.
    pub intent: String,
    pub must_read: Vec<ModuleId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<ModuleId>,
    /// Boundaries. Present in every bundle for this route regardless of budget or relevance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_omittable: Vec<ModuleId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_outputs: Vec<String>,
    /// Estimated-token ceiling. `None` is unbounded, not "default".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<u32>,
}

impl TaskRoute {
    pub fn new(id: RouteId, intent: impl Into<String>) -> Self {
        TaskRoute {
            id,
            intent: intent.into(),
            must_read: Vec::new(),
            optional: Vec::new(),
            non_omittable: Vec::new(),
            expected_outputs: Vec::new(),
            budget: None,
        }
    }

    pub fn must_read(mut self, module: ModuleId) -> Self {
        self.must_read.push(module);
        self
    }

    pub fn optional(mut self, module: ModuleId) -> Self {
        self.optional.push(module);
        self
    }

    pub fn non_omittable(mut self, module: ModuleId) -> Self {
        self.non_omittable.push(module);
        self
    }

    pub fn with_budget(mut self, budget: u32) -> Self {
        self.budget = Some(budget);
        self
    }

    pub fn expecting(mut self, output: impl Into<String>) -> Self {
        self.expected_outputs.push(output.into());
        self
    }

    /// Every module the route names, deduplicated and ordered.
    pub fn declared_modules(&self) -> BTreeSet<ModuleId> {
        self.must_read
            .iter()
            .chain(self.optional.iter())
            .chain(self.non_omittable.iter())
            .cloned()
            .collect()
    }

    /// The nodes a bundle starts from before any closure rule runs.
    pub fn seeds(&self) -> Vec<ModuleId> {
        let mut seeds: Vec<ModuleId> = self.must_read.clone();
        for module in &self.non_omittable {
            if !seeds.contains(module) {
                seeds.push(module.clone());
            }
        }
        seeds
    }

    /// The 41.05 invariants, as data.
    ///
    /// Returns findings rather than `Result` so an authoring tool can show all of them at once;
    /// [`crate::bundle`] turns the ones that would corrupt a delivered context into hard errors.
    pub fn check(&self, graph: &DocGraph) -> Vec<RouteDefect> {
        let mut defects = Vec::new();
        if self.must_read.is_empty() {
            defects.push(RouteDefect::NoMustRead);
        }
        for module in self.declared_modules() {
            if !graph.contains(&module) {
                defects.push(RouteDefect::MissingModule { module });
            }
        }
        for module in &self.optional {
            if self.non_omittable.contains(module) {
                defects.push(RouteDefect::NonOmittableListedAsOptional {
                    module: module.clone(),
                });
            }
        }
        let components = must_read_components(self, graph);
        if components.len() > 1 {
            defects.push(RouteDefect::MustReadNotConnected { components });
        }
        defects
    }
}

/// Connected components of the must-read set, over edges in either direction.
///
/// Direction is ignored here on purpose. 41.05 asks whether the must-read set hangs together as a
/// *subject*, and `A requires B` and `B provides A` connect their endpoints equally well for that
/// question; a directed reading would report a route as disconnected purely because its author
/// happened to draw the arrows outward from the leaf.
fn must_read_components(route: &TaskRoute, graph: &DocGraph) -> Vec<Vec<ModuleId>> {
    let present: BTreeSet<ModuleId> = route
        .must_read
        .iter()
        .filter(|module| graph.contains(module))
        .cloned()
        .collect();
    let mut unassigned = present.clone();
    let mut components = Vec::new();
    while let Some(seed) = unassigned.iter().next().cloned() {
        let mut component = BTreeSet::new();
        let mut frontier = vec![seed];
        while let Some(current) = frontier.pop() {
            if !unassigned.remove(&current) {
                continue;
            }
            component.insert(current.clone());
            for edge in graph.edges() {
                let neighbor = if edge.from == current {
                    &edge.to
                } else if edge.to == current {
                    &edge.from
                } else {
                    continue;
                };
                if unassigned.contains(neighbor) {
                    frontier.push(neighbor.clone());
                }
            }
        }
        components.push(component.into_iter().collect());
    }
    components
}
