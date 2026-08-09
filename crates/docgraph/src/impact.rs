//! Change-impact analysis (41.10).
//!
//! Given a module that changed, which other modules, routes and already-compiled bundles are no
//! longer trustworthy. 41.10 requires that "impact results are conservative" and that "schema and
//! governance changes always include downstream release nodes".
//!
//! # Impact runs against the edges
//!
//! Every edge type in [`crate::vocabulary`] is read the same way: an edge `A --kind--> B` means A
//! was written with B in view, so a change to B reaches A. Impact therefore walks *incoming*
//! edges — from the changed module to its dependents — with one exception,
//! [`DocEdgeType::Contradicts`], which is symmetric and is walked both ways. No edge type needs
//! its own direction rule, which is the property that made it worth typing the vocabulary in the
//! first place.
//!
//! # The asymmetry: transitive through `requires` and `refines`, not through `example_of`
//!
//! [`ImpactPropagation`] decides whether a hop continues. `requires`, `provides`, `governs`,
//! `implements`, `supersedes` and `refines` continue; `references`, `part_of`, `contains`,
//! `evaluates`, `contradicts` and `example_of` stop after one hop.
//!
//! The case worth defending is `example_of`. A contract's dependents are themselves contracts —
//! things are written against *them* — so a change genuinely travels down that chain, and
//! stopping early would miss a real obligation. An example is a leaf: it illustrates a contract
//! and nothing is written against the illustration. A document that cites an example cites it as
//! an illustration, and its correctness rests on the contract, which the walk already reached
//! directly. Making `example_of` transitive would let one contract edit reach every document that
//! ever mentioned an example of it, and 41.10's "conservative" is a floor on recall, not a licence
//! to return the corpus: a report that names everything tells a reviewer nothing and gets
//! ignored, which is a worse failure than a report that is narrow and says where it stopped.
//!
//! That last clause is the honest part. Every hop the walk declined to continue is recorded in
//! [`ImpactReport::stopped_at`] with the edge type responsible, so "we stopped here" is in the
//! artifact rather than in this comment.
//!
//! # Not implemented
//!
//! No severity ranking of affected modules and no diff awareness — this analysis knows that a
//! module changed, not what changed inside it. A whitespace fix and a rewritten invariant produce
//! the same report. Narrowing on that would need a semantic diff over normative text, which is a
//! different crate and would be guesswork here.

use crate::bundle::ContextBundle;
use crate::registry::{DocGraph, ModuleId};
use crate::route::{RouteId, TaskRoute};
use crate::vocabulary::{DocEdgeType, ImpactPropagation};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One step of the impact walk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactHop {
    pub module: ModuleId,
    /// The module whose change reached it.
    pub from: ModuleId,
    pub via: DocEdgeType,
    pub depth: u32,
    /// Whether the walk continued past this module.
    pub continued: bool,
}

/// A hop the walk deliberately did not continue past.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropagationStop {
    pub module: ModuleId,
    pub via: DocEdgeType,
    /// The gloss of the edge type, so the report explains itself.
    pub because: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    pub changed: ModuleId,
    /// Every affected module, ordered by id.
    pub affected: Vec<ImpactHop>,
    /// Where transitivity was declined, and via which edge type.
    pub stopped_at: Vec<PropagationStop>,
    /// Routes naming the changed module or anything affected.
    pub affected_routes: Vec<RouteId>,
}

impl ImpactReport {
    /// Affected modules plus the changed module itself.
    pub fn closure(&self) -> BTreeSet<ModuleId> {
        let mut closure: BTreeSet<ModuleId> = self
            .affected
            .iter()
            .map(|hop| hop.module.clone())
            .collect();
        closure.insert(self.changed.clone());
        closure
    }

    pub fn touches(&self, module: &ModuleId) -> bool {
        &self.changed == module || self.affected.iter().any(|hop| &hop.module == module)
    }

    /// Bundle entries this change invalidates.
    ///
    /// An entry is invalidated when its module is in the closure — including entries admitted at
    /// [`ProfileLevel::Card`](crate::tokens::ProfileLevel::Card), because a card is rendered from
    /// registry fields that the change may have moved.
    pub fn invalidated_entries<'a>(&self, bundle: &'a ContextBundle) -> Vec<&'a ModuleId> {
        let closure = self.closure();
        bundle
            .module_ids()
            .filter(|module| closure.contains(*module))
            .collect()
    }

    pub fn invalidates(&self, bundle: &ContextBundle) -> bool {
        !self.invalidated_entries(bundle).is_empty()
    }
}

/// Compute the impact of a change to `changed`.
///
/// Deterministic: the frontier is sorted at every level, so the emitted hop for a module reached
/// by two paths at the same depth is the one whose source id sorts first.
pub fn impact_of(graph: &DocGraph, changed: &ModuleId, routes: &[TaskRoute]) -> ImpactReport {
    let mut hops: BTreeMap<ModuleId, ImpactHop> = BTreeMap::new();
    let mut expanded: BTreeSet<ModuleId> = BTreeSet::new();
    let mut stops: BTreeMap<(ModuleId, DocEdgeType), PropagationStop> = BTreeMap::new();
    let mut queue: VecDeque<(ModuleId, u32)> = VecDeque::new();

    queue.push_back((changed.clone(), 0));
    expanded.insert(changed.clone());

    while let Some((current, depth)) = queue.pop_front() {
        let mut dependents: Vec<(ModuleId, DocEdgeType)> = Vec::new();
        for edge in graph.edges() {
            if edge.to == current {
                dependents.push((edge.from.clone(), edge.kind));
            } else if edge.kind.symmetric() && edge.from == current {
                dependents.push((edge.to.clone(), edge.kind));
            }
        }
        dependents.sort();
        dependents.dedup();

        for (dependent, kind) in dependents {
            if &dependent == changed || !graph.contains(&dependent) {
                continue;
            }
            let continues = kind.propagation() == ImpactPropagation::Transitive;
            let entry = hops.entry(dependent.clone()).or_insert(ImpactHop {
                module: dependent.clone(),
                from: current.clone(),
                via: kind,
                depth: depth + 1,
                continued: false,
            });
            if continues {
                entry.continued = true;
            } else {
                stops
                    .entry((dependent.clone(), kind))
                    .or_insert(PropagationStop {
                        module: dependent.clone(),
                        via: kind,
                        because: kind.gloss().to_string(),
                    });
            }
            if continues && expanded.insert(dependent.clone()) {
                queue.push_back((dependent, depth + 1));
            }
        }
    }

    let affected: Vec<ImpactHop> = hops.into_values().collect();
    let closure: BTreeSet<ModuleId> = affected
        .iter()
        .map(|hop| hop.module.clone())
        .chain(std::iter::once(changed.clone()))
        .collect();

    let mut affected_routes: Vec<RouteId> = routes
        .iter()
        .filter(|route| {
            route
                .declared_modules()
                .iter()
                .any(|module| closure.contains(module))
        })
        .map(|route| route.id.clone())
        .collect();
    affected_routes.sort();
    affected_routes.dedup();

    ImpactReport {
        changed: changed.clone(),
        affected,
        stopped_at: stops
            .into_values()
            .filter(|stop| !expanded.contains(&stop.module))
            .collect(),
        affected_routes,
    }
}
