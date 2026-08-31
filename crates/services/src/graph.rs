//! Who may call whom, and who owns what.
//!
//! Implements blueprint 40.03 (Service and Process Graph) and 40.04 (Domain Boundaries and
//! Ownership). 40.04 ships a nine-row table of *owns* and *does not own*; 40.03 ships two mermaid
//! diagrams. Neither is checkable as drawn, and the difference between a diagram and a contract is
//! that a contract refuses something.
//!
//! Three things are refused here.
//!
//! **A call that leaves its domain without a declared contract.** Inside a domain, code may call
//! code; that is what a domain is. The moment a call crosses into another domain it is an
//! integration, and 40.04's closing sentence — cross-domain changes require an ADR and conformance
//! updates — is meaningless if nobody can enumerate the crossings. [`GraphError::UndeclaredCrossing`]
//! is that enumeration made mandatory.
//!
//! **A cycle.** Reported with the cycle named, in a rotation-canonical order so that the same cycle
//! is reported the same way whichever node the search entered from.
//!
//! **An ownership table that does not add up.** A concern owned by two domains has no owner in
//! practice, and a concern one domain disclaims that no domain owns is a hole where an ADR should
//! be.
//!
//! # A cycle the blueprint itself draws
//!
//! 40.03's alpha topology contains `CORE --> EX` and `EX --> CORE`. Read as a call graph that is a
//! two-cycle, and a checker that forbade cycles outright would reject the blueprint's own diagram
//! on its first run. It is not a cycle: the second arrow is the executor's result returning along
//! the call it was invoked by. So an edge carries an [`EdgeKind`], cycles are computed over
//! [`EdgeKind::Request`] edges only, and a [`EdgeKind::Result`] edge with no request to return
//! along is itself an error. The distinction is not in 40.03; it has to be added to make 40.03
//! consistent with itself.
//!
//! # What is deliberately not implemented
//!
//! No transport, no service discovery, no health checking, no deployment. A node here is a named
//! responsibility, not a process, and 40.03 is explicit that the same services run in-process and
//! behind a boundary — see [`crate::topology`]. Nothing in this module knows whether anything is
//! running.

use crate::contract::ContractId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A named responsibility in the graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServiceId(String);

impl ServiceId {
    pub fn new(name: impl Into<String>) -> Self {
        ServiceId(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Something a domain is accountable for, named as 40.04 names it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Concern(String);

impl Concern {
    pub fn new(name: impl Into<String>) -> Self {
        Concern(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Concern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The nine domains of 40.04, in table order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Domain {
    BiologicalIr,
    World,
    Context,
    Runtime,
    Compiler,
    Oracle,
    Evaluator,
    Registry,
    OncoWorld,
}

impl Domain {
    pub const ALL: [Domain; 9] = [
        Domain::BiologicalIr,
        Domain::World,
        Domain::Context,
        Domain::Runtime,
        Domain::Compiler,
        Domain::Oracle,
        Domain::Evaluator,
        Domain::Registry,
        Domain::OncoWorld,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Domain::BiologicalIr => "Biological IR",
            Domain::World => "World",
            Domain::Context => "Context",
            Domain::Runtime => "Runtime",
            Domain::Compiler => "Compiler",
            Domain::Oracle => "Oracle",
            Domain::Evaluator => "Evaluator",
            Domain::Registry => "Registry",
            Domain::OncoWorld => "OncoWorld",
        }
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One entry from 40.04's "Does not own" column.
///
/// The column mixes two kinds of statement and the difference matters. *"World does not own model
/// provider logic"* names a concern that belongs to some other domain and is therefore checkable:
/// somebody must own it. *"Oracle does not own hiding implementation failures"* names a behaviour
/// nobody may do, which no structural check can reach at all. Collapsing them would let the second
/// kind be reported as satisfied because it was never checked.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Disclaimer {
    /// A concern this domain does not own, and some other domain must.
    OwnedElsewhere { concern: Concern },
    /// A behaviour forbidden to this domain. Recorded, not checked: nothing structural can observe
    /// a service hiding a failure.
    Prohibited { behaviour: String },
}

/// A domain's row in 40.04's table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ownership {
    pub domain: Domain,
    pub owns: Vec<Concern>,
    pub disclaims: Vec<Disclaimer>,
}

/// Whether an edge carries a request or the answer to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// A calls B. Subject to the boundary rule and counted in cycle detection.
    Request,
    /// B answers A along a request A already made. Not a call, and not a cycle.
    Result,
}

/// One edge of the service graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub caller: ServiceId,
    pub callee: ServiceId,
    pub kind: EdgeKind,
    /// The contract the call is made under. Required whenever the edge leaves its domain.
    pub contract: Option<ContractId>,
}

impl Call {
    /// A request made under a declared contract.
    pub fn under(caller: &ServiceId, callee: &ServiceId, contract: ContractId) -> Self {
        Call {
            caller: caller.clone(),
            callee: callee.clone(),
            kind: EdgeKind::Request,
            contract: Some(contract),
        }
    }

    /// A request with no declared contract. Legal within a domain, an error across one.
    pub fn internal(caller: &ServiceId, callee: &ServiceId) -> Self {
        Call {
            caller: caller.clone(),
            callee: callee.clone(),
            kind: EdgeKind::Request,
            contract: None,
        }
    }

    /// The answer returning along an existing request.
    pub fn result(caller: &ServiceId, callee: &ServiceId) -> Self {
        Call {
            caller: caller.clone(),
            callee: callee.clone(),
            kind: EdgeKind::Result,
            contract: None,
        }
    }
}

/// A service and the domain it answers to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceNode {
    pub id: ServiceId,
    pub domain: Domain,
    /// The workspace crate that implements it, or `None` where nothing does.
    pub implemented_by: Option<String>,
    /// The §40 contract this service is the subject of, where one of the nine covers it.
    pub contract: Option<ContractId>,
}

/// Everything wrong with a graph, as a type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    #[error("service id {service:?} is empty or contains a control character")]
    InvalidServiceId { service: ServiceId },

    #[error("service id {service} appears more than once in the graph")]
    DuplicateService { service: ServiceId },

    #[error("{edge} names {missing}, which is not a service in this graph")]
    UnknownService { edge: String, missing: ServiceId },

    #[error(
        "{caller} ({from}) calls {callee} ({to}) with no declared contract; a call that leaves its \
         domain is an integration and 40.04 requires an ADR and a conformance update for it"
    )]
    UndeclaredCrossing {
        caller: ServiceId,
        callee: ServiceId,
        from: Domain,
        to: Domain,
    },

    #[error(
        "{caller} calls {callee} under {contract}, which is not the contract {callee} answers"
    )]
    WrongContract {
        caller: ServiceId,
        callee: ServiceId,
        contract: ContractId,
    },

    #[error("cycle in the call graph: {}", .path.iter().map(ServiceId::as_str).collect::<Vec<_>>().join(" -> "))]
    Cycle { path: Vec<ServiceId> },

    #[error("{callee} returns a result to {caller}, which never calls it")]
    UnpairedResult {
        caller: ServiceId,
        callee: ServiceId,
    },

    #[error("concern {concern:?} is owned by {} domains: {}", .domains.len(), .domains.iter().map(|domain| domain.as_str()).collect::<Vec<_>>().join(", "))]
    OwnershipConflict {
        concern: Concern,
        domains: Vec<Domain>,
    },

    #[error(
        "{domain} disclaims {concern:?} and no domain in the table owns it, so the boundary names \
         a responsibility nobody has"
    )]
    OrphanedConcern { domain: Domain, concern: Concern },

    #[error("{domain} both owns and disclaims {concern:?}")]
    ContradictoryOwnership { domain: Domain, concern: Concern },

    #[error("service {service} is assigned to {domain}, which owns nothing in this table")]
    DomainNotDescribed { service: ServiceId, domain: Domain },
}

/// A service graph with its ownership table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceGraph {
    pub nodes: Vec<ServiceNode>,
    pub calls: Vec<Call>,
    pub ownership: Vec<Ownership>,
}

impl ServiceGraph {
    pub fn new(nodes: Vec<ServiceNode>, calls: Vec<Call>, ownership: Vec<Ownership>) -> Self {
        ServiceGraph {
            nodes,
            calls,
            ownership,
        }
    }

    pub fn node(&self, id: &ServiceId) -> Option<&ServiceNode> {
        self.nodes.iter().find(|node| &node.id == id)
    }

    fn domain_of(&self, id: &ServiceId) -> Option<Domain> {
        self.node(id).map(|node| node.domain)
    }

    /// Every finding, in a deterministic order: nodes and edges first, then ownership.
    ///
    /// Does not stop at the first problem. A graph with two undeclared crossings and a cycle is
    /// three separate conversations, and reporting one at a time turns a review into a queue.
    pub fn audit(&self) -> Vec<GraphError> {
        let mut findings = Vec::new();
        findings.extend(self.audit_nodes());
        findings.extend(self.audit_edges());
        findings.extend(self.cycles());
        findings.extend(self.audit_ownership());
        findings
    }

    /// The first finding, for a caller that only needs a yes or no.
    pub fn check(&self) -> Result<(), GraphError> {
        match self.audit().into_iter().next() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn audit_nodes(&self) -> Vec<GraphError> {
        let mut findings = Vec::new();
        let mut ids = BTreeSet::new();
        for node in &self.nodes {
            if node.id.as_str().trim().is_empty() || node.id.as_str().chars().any(char::is_control)
            {
                findings.push(GraphError::InvalidServiceId {
                    service: node.id.clone(),
                });
            }
            if !ids.insert(node.id.clone()) {
                findings.push(GraphError::DuplicateService {
                    service: node.id.clone(),
                });
            }
        }
        findings
    }

    fn audit_edges(&self) -> Vec<GraphError> {
        let mut findings = Vec::new();
        let requests: BTreeSet<(&ServiceId, &ServiceId)> = self
            .calls
            .iter()
            .filter(|call| call.kind == EdgeKind::Request)
            .map(|call| (&call.caller, &call.callee))
            .collect();

        for call in &self.calls {
            let label = format!("{} -> {}", call.caller, call.callee);
            let from = match self.domain_of(&call.caller) {
                Some(domain) => domain,
                None => {
                    findings.push(GraphError::UnknownService {
                        edge: label.clone(),
                        missing: call.caller.clone(),
                    });
                    continue;
                }
            };
            let to = match self.domain_of(&call.callee) {
                Some(domain) => domain,
                None => {
                    findings.push(GraphError::UnknownService {
                        edge: label,
                        missing: call.callee.clone(),
                    });
                    continue;
                }
            };

            match call.kind {
                EdgeKind::Request => {
                    if from != to && call.contract.is_none() {
                        findings.push(GraphError::UndeclaredCrossing {
                            caller: call.caller.clone(),
                            callee: call.callee.clone(),
                            from,
                            to,
                        });
                    }
                    if let Some(contract) = call.contract {
                        let answered = self.node(&call.callee).and_then(|node| node.contract);
                        if answered != Some(contract) {
                            findings.push(GraphError::WrongContract {
                                caller: call.caller.clone(),
                                callee: call.callee.clone(),
                                contract,
                            });
                        }
                    }
                }
                EdgeKind::Result => {
                    if !requests.contains(&(&call.callee, &call.caller)) {
                        findings.push(GraphError::UnpairedResult {
                            caller: call.caller.clone(),
                            callee: call.callee.clone(),
                        });
                    }
                }
            }
        }
        findings
    }

    /// Every elementary cycle over request edges, each named by its members.
    ///
    /// Canonicalised by rotating each cycle so that its smallest member comes first, which makes
    /// the report independent of where the search started.
    pub fn cycles(&self) -> Vec<GraphError> {
        let mut adjacency: BTreeMap<&ServiceId, BTreeSet<&ServiceId>> = BTreeMap::new();
        for node in &self.nodes {
            adjacency.entry(&node.id).or_default();
        }
        for call in &self.calls {
            if call.kind == EdgeKind::Request {
                if let Some(targets) = adjacency.get_mut(&call.caller) {
                    if self.node(&call.callee).is_some() {
                        targets.insert(&call.callee);
                    }
                }
            }
        }

        let mut found: BTreeSet<Vec<ServiceId>> = BTreeSet::new();
        let starts: Vec<&ServiceId> = adjacency.keys().copied().collect();
        for start in starts {
            let mut path = vec![start];
            walk(&adjacency, start, &mut path, &mut found);
        }

        found
            .into_iter()
            .map(|path| GraphError::Cycle { path })
            .collect()
    }

    fn audit_ownership(&self) -> Vec<GraphError> {
        let mut findings = Vec::new();
        let mut owners: BTreeMap<&Concern, Vec<Domain>> = BTreeMap::new();
        for row in &self.ownership {
            for concern in &row.owns {
                owners.entry(concern).or_default().push(row.domain);
            }
        }

        for (concern, domains) in &owners {
            if domains.len() > 1 {
                findings.push(GraphError::OwnershipConflict {
                    concern: (*concern).clone(),
                    domains: domains.clone(),
                });
            }
        }

        for row in &self.ownership {
            for disclaimer in &row.disclaims {
                let Disclaimer::OwnedElsewhere { concern } = disclaimer else {
                    continue;
                };
                if row.owns.contains(concern) {
                    findings.push(GraphError::ContradictoryOwnership {
                        domain: row.domain,
                        concern: concern.clone(),
                    });
                } else if !owners.contains_key(concern) {
                    findings.push(GraphError::OrphanedConcern {
                        domain: row.domain,
                        concern: concern.clone(),
                    });
                }
            }
        }

        let described: BTreeSet<Domain> = self.ownership.iter().map(|row| row.domain).collect();
        for node in &self.nodes {
            if !described.contains(&node.domain) {
                findings.push(GraphError::DomainNotDescribed {
                    service: node.id.clone(),
                    domain: node.domain,
                });
            }
        }

        findings
    }

    /// The concerns 40.04 says are somebody else's and that nobody in the table claims.
    pub fn orphaned_concerns(&self) -> Vec<Concern> {
        self.audit_ownership()
            .into_iter()
            .filter_map(|finding| match finding {
                GraphError::OrphanedConcern { concern, .. } => Some(concern),
                _ => None,
            })
            .collect()
    }

    /// The cross-domain calls that have no contract among the nine.
    pub fn undeclared_crossings(&self) -> Vec<(ServiceId, ServiceId)> {
        self.audit_edges()
            .into_iter()
            .filter_map(|finding| match finding {
                GraphError::UndeclaredCrossing { caller, callee, .. } => Some((caller, callee)),
                _ => None,
            })
            .collect()
    }
}

fn walk<'a>(
    adjacency: &BTreeMap<&'a ServiceId, BTreeSet<&'a ServiceId>>,
    current: &'a ServiceId,
    path: &mut Vec<&'a ServiceId>,
    found: &mut BTreeSet<Vec<ServiceId>>,
) {
    let Some(targets) = adjacency.get(current) else {
        return;
    };
    for next in targets {
        if let Some(position) = path.iter().position(|seen| seen == next) {
            let cycle: Vec<ServiceId> = path[position..].iter().map(|id| (*id).clone()).collect();
            found.insert(canonical_rotation(cycle));
        } else if path.len() < adjacency.len() {
            path.push(next);
            walk(adjacency, next, path, found);
            path.pop();
        }
    }
}

fn canonical_rotation(cycle: Vec<ServiceId>) -> Vec<ServiceId> {
    let Some(pivot) = cycle
        .iter()
        .enumerate()
        .min_by_key(|(_, id)| (*id).clone())
        .map(|(index, _)| index)
    else {
        return cycle;
    };
    let mut rotated = cycle[pivot..].to_vec();
    rotated.extend_from_slice(&cycle[..pivot]);
    rotated
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, domain: Domain) -> ServiceNode {
        ServiceNode {
            id: ServiceId::new(name),
            domain,
            implemented_by: None,
            contract: None,
        }
    }

    fn answering(name: &str, domain: Domain, contract: ContractId) -> ServiceNode {
        ServiceNode {
            contract: Some(contract),
            ..node(name, domain)
        }
    }

    fn table() -> Vec<Ownership> {
        vec![
            Ownership {
                domain: Domain::World,
                owns: vec![Concern::new("visibility")],
                disclaims: Vec::new(),
            },
            Ownership {
                domain: Domain::Context,
                owns: vec![Concern::new("projection")],
                disclaims: Vec::new(),
            },
            Ownership {
                domain: Domain::Runtime,
                owns: vec![Concern::new("sandbox actions")],
                disclaims: Vec::new(),
            },
        ]
    }

    #[test]
    fn a_call_crossing_an_ownership_boundary_without_a_contract_is_an_error() {
        let graph = ServiceGraph::new(
            vec![
                node("context-compiler", Domain::Context),
                node("world-builder", Domain::World),
            ],
            vec![Call::internal(
                &ServiceId::new("context-compiler"),
                &ServiceId::new("world-builder"),
            )],
            table(),
        );
        let findings = graph.audit();
        assert!(matches!(
            findings.as_slice(),
            [GraphError::UndeclaredCrossing {
                from: Domain::Context,
                to: Domain::World,
                ..
            }]
        ));
    }

    #[test]
    fn a_call_inside_one_domain_needs_no_contract() {
        let graph = ServiceGraph::new(
            vec![
                node("decision-compiler", Domain::Context),
                node("expander", Domain::Context),
            ],
            vec![Call::internal(
                &ServiceId::new("decision-compiler"),
                &ServiceId::new("expander"),
            )],
            table(),
        );
        assert_eq!(graph.audit(), Vec::new());
    }

    #[test]
    fn a_declared_crossing_is_accepted_when_the_callee_answers_that_contract() {
        let graph = ServiceGraph::new(
            vec![
                node("context-compiler", Domain::Context),
                answering("world-builder", Domain::World, ContractId::WorldBuilder),
            ],
            vec![Call::under(
                &ServiceId::new("context-compiler"),
                &ServiceId::new("world-builder"),
                ContractId::WorldBuilder,
            )],
            table(),
        );
        assert_eq!(graph.audit(), Vec::new());
    }

    #[test]
    fn a_call_declaring_a_contract_the_callee_does_not_answer_is_an_error() {
        let graph = ServiceGraph::new(
            vec![
                node("context-compiler", Domain::Context),
                answering("world-builder", Domain::World, ContractId::WorldBuilder),
            ],
            vec![Call::under(
                &ServiceId::new("context-compiler"),
                &ServiceId::new("world-builder"),
                ContractId::RegistryBackend,
            )],
            table(),
        );
        assert!(matches!(
            graph.audit().as_slice(),
            [GraphError::WrongContract { .. }]
        ));
    }

    #[test]
    fn a_cycle_is_reported_with_every_service_on_it_named() {
        let graph = ServiceGraph::new(
            vec![
                node("a", Domain::Context),
                node("b", Domain::Context),
                node("c", Domain::Context),
            ],
            vec![
                Call::internal(&ServiceId::new("a"), &ServiceId::new("b")),
                Call::internal(&ServiceId::new("b"), &ServiceId::new("c")),
                Call::internal(&ServiceId::new("c"), &ServiceId::new("a")),
            ],
            table(),
        );
        let cycles = graph.cycles();
        assert_eq!(cycles.len(), 1);
        let GraphError::Cycle { path } = &cycles[0] else {
            panic!("expected a cycle");
        };
        assert_eq!(
            path.iter().map(ServiceId::as_str).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert!(cycles[0].to_string().contains("a -> b -> c"));
    }

    #[test]
    fn the_same_cycle_is_reported_once_however_the_search_enters_it() {
        let graph = ServiceGraph::new(
            vec![
                node("z", Domain::Context),
                node("y", Domain::Context),
                node("x", Domain::Context),
            ],
            vec![
                Call::internal(&ServiceId::new("z"), &ServiceId::new("y")),
                Call::internal(&ServiceId::new("y"), &ServiceId::new("x")),
                Call::internal(&ServiceId::new("x"), &ServiceId::new("z")),
            ],
            table(),
        );
        assert_eq!(graph.cycles().len(), 1);
    }

    #[test]
    fn a_result_edge_returning_along_a_request_is_not_a_cycle() {
        let graph = ServiceGraph::new(
            vec![
                node("core", Domain::Runtime),
                node("executor", Domain::Runtime),
            ],
            vec![
                Call::internal(&ServiceId::new("core"), &ServiceId::new("executor")),
                Call::result(&ServiceId::new("executor"), &ServiceId::new("core")),
            ],
            table(),
        );
        assert_eq!(graph.cycles(), Vec::new());
        assert_eq!(graph.audit(), Vec::new());
    }

    #[test]
    fn a_result_edge_with_no_request_to_return_along_is_an_error() {
        let graph = ServiceGraph::new(
            vec![
                node("core", Domain::Runtime),
                node("executor", Domain::Runtime),
            ],
            vec![Call::result(
                &ServiceId::new("executor"),
                &ServiceId::new("core"),
            )],
            table(),
        );
        assert!(matches!(
            graph.audit().as_slice(),
            [GraphError::UnpairedResult { .. }]
        ));
    }

    #[test]
    fn an_edge_naming_a_service_the_graph_does_not_contain_is_an_error() {
        let graph = ServiceGraph::new(
            vec![node("a", Domain::Context)],
            vec![Call::internal(
                &ServiceId::new("a"),
                &ServiceId::new("ghost"),
            )],
            table(),
        );
        assert!(matches!(
            graph.audit().as_slice(),
            [GraphError::UnknownService { .. }]
        ));
    }

    #[test]
    fn duplicate_and_blank_service_ids_are_rejected_before_edge_analysis() {
        let graph = ServiceGraph::new(
            vec![
                node("duplicate", Domain::Context),
                node("duplicate", Domain::World),
                node("\t", Domain::Runtime),
            ],
            Vec::new(),
            table(),
        );
        let findings = graph.audit();
        assert!(findings.iter().any(|finding| matches!(
            finding,
            GraphError::DuplicateService { service } if service.as_str() == "duplicate"
        )));
        assert!(findings.iter().any(|finding| matches!(
            finding,
            GraphError::InvalidServiceId { service } if service.as_str() == "\t"
        )));
    }

    #[test]
    fn a_concern_owned_by_two_domains_is_owned_by_neither() {
        let graph = ServiceGraph::new(
            Vec::new(),
            Vec::new(),
            vec![
                Ownership {
                    domain: Domain::World,
                    owns: vec![Concern::new("reveal")],
                    disclaims: Vec::new(),
                },
                Ownership {
                    domain: Domain::Context,
                    owns: vec![Concern::new("reveal")],
                    disclaims: Vec::new(),
                },
            ],
        );
        assert!(matches!(
            graph.audit().as_slice(),
            [GraphError::OwnershipConflict { .. }]
        ));
    }

    #[test]
    fn a_disclaimed_concern_no_domain_owns_is_reported_as_orphaned() {
        let graph = ServiceGraph::new(
            Vec::new(),
            Vec::new(),
            vec![Ownership {
                domain: Domain::World,
                owns: vec![Concern::new("reveal")],
                disclaims: vec![Disclaimer::OwnedElsewhere {
                    concern: Concern::new("model provider logic"),
                }],
            }],
        );
        assert_eq!(
            graph.orphaned_concerns(),
            [Concern::new("model provider logic")]
        );
    }

    #[test]
    fn a_prohibited_behaviour_is_recorded_and_never_reported_as_orphaned() {
        let graph = ServiceGraph::new(
            Vec::new(),
            Vec::new(),
            vec![Ownership {
                domain: Domain::Oracle,
                owns: vec![Concern::new("evidence establishment and uncertainty")],
                disclaims: vec![Disclaimer::Prohibited {
                    behaviour: "hiding implementation failures".to_string(),
                }],
            }],
        );
        assert_eq!(graph.audit(), Vec::new());
        assert_eq!(graph.orphaned_concerns(), Vec::new());
    }

    #[test]
    fn a_domain_that_both_owns_and_disclaims_a_concern_is_contradictory() {
        let graph = ServiceGraph::new(
            Vec::new(),
            Vec::new(),
            vec![Ownership {
                domain: Domain::World,
                owns: vec![Concern::new("reveal")],
                disclaims: vec![Disclaimer::OwnedElsewhere {
                    concern: Concern::new("reveal"),
                }],
            }],
        );
        assert!(matches!(
            graph.audit().as_slice(),
            [GraphError::ContradictoryOwnership { .. }]
        ));
    }

    #[test]
    fn an_audit_reports_every_finding_rather_than_stopping_at_the_first() {
        let graph = ServiceGraph::new(
            vec![
                node("a", Domain::Context),
                node("b", Domain::World),
                node("c", Domain::World),
            ],
            vec![
                Call::internal(&ServiceId::new("a"), &ServiceId::new("b")),
                Call::internal(&ServiceId::new("b"), &ServiceId::new("c")),
                Call::internal(&ServiceId::new("c"), &ServiceId::new("b")),
            ],
            table(),
        );
        let findings = graph.audit();
        assert_eq!(
            findings.len(),
            2,
            "one crossing and one cycle: {findings:?}"
        );
        assert!(graph.check().is_err());
    }
}
