//! The multi-agent separator protocol: blueprint 43.29.
//!
//! > Coordinate specialist agents by exchanging typed separator messages and local obstruction
//! > reports rather than full transcripts or global context copies.
//!
//! ## What this module is, and what it deliberately is not
//!
//! `bioprism-weave` already owns the capsule. Continuations, authority leases with transitive
//! revocation, affine budgets that cannot be duplicated because `Budget` does not implement
//! `Clone`, the commitment and epistemic ledgers, and the transport itself are all its. `bioprism-
//! fabric` owns the layer above: the composition algebra, agent-to-agent information flow, and the
//! contract net. **This module builds neither a second capsule nor a second transport.**
//!
//! What is left over is the part those two crates cannot answer, because it is a question about
//! the *factorisation of the problem* rather than about the agents: given a partition of a factor
//! graph across agents, which variables must actually cross the boundary, is one collection pass
//! exact, and what does the traffic cost relative to sharing the transcript. Concretely, the
//! division is:
//!
//! | Concern from 43.29's runtime contract | Owner |
//! |---|---|
//! | Agent local fiber — assigned factors and scopes | here ([`Partition`]) |
//! | Separator schema — only shared variables | here ([`separator`], enforced in [`SeparatorMessage::new`]) |
//! | Message — algebraic content, provenance, residual, validity key | here ([`SeparatorMessage`]) |
//! | Obstruction — a local inconsistency that will not compress into a message | here ([`Obstruction`]) |
//! | Convergence and stopping | here for the exact tree case only; loopy is labelled, not iterated |
//! | Agent tools, authority, budgets, leases | `bioprism-weave` |
//! | Who must produce, challenge or verify a message | `bioprism-weave`'s commitment ledger |
//! | Policy screening of what may be sent | `bioprism-policy` |
//!
//! ## The leak rule is a type, not a review
//!
//! 43.29's first non-negotiable invariant is that "agents do not receive unrelated private
//! subworlds". [`SeparatorMessage::new`] refuses any message whose scope is not exactly the
//! separator. A variable that is local to the sender cannot be in a message even as a harmless
//! extra field, because "harmless" is a judgement and the whole point of a separator is that the
//! judgement was already made structurally.
//!
//! ## Exactness is checked against a centralised answer, not asserted
//!
//! 43.47's theorem candidate C says junction-tree message passing recovers the exact aggregate and
//! that "loopy message passing has no such blanket claim". [`structure`] decides which case a
//! partition is in, [`collect`] runs one collection pass, and [`centralised`] computes the same
//! marginal by enumerating every joint assignment. The suite asserts they agree on trees — and
//! asserts they *disagree* on a cycle, because a protocol that happened to be right on loops would
//! mean the test world was too weak to tell the two regimes apart.
//!
//! ## What is not implemented
//!
//! Iterative loopy passing, damping, residual scheduling, and any convergence criterion. A loopy
//! partition here gets [`Exactness::LoopyApproximate`] and one pass. 43.29's fallback list also
//! includes repartitioning when separators get large; [`TransportCost`] measures when that has
//! happened and does not act on it.

use crate::error::EpistemicError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Largest local assignment space a single agent may have. `2^16`.
pub const MAX_LOCAL_ASSIGNMENTS: usize = 1 << 16;

/// Largest joint assignment space [`centralised`] will enumerate. `2^20`.
pub const MAX_JOINT_ASSIGNMENTS: usize = 1 << 20;

/// A factor over binary variables, as a full table.
///
/// Binary because the point here is the *structure* of the exchange, and richer domains would add
/// index arithmetic without adding a claim. Entry order: for scope `[v0, v1, …]`, the assignment
/// with `v_k = x_k` lives at `Σ x_k · 2^k`.
///
/// `bioprism-world`'s factors carry a signature and no potential — that absence is the finding
/// `bioprism-influence` reports — so these tables are supplied by the caller rather than read out
/// of a world. A `fiber-world/0.1` world cannot express one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactorTable {
    pub id: String,
    pub scope: Vec<String>,
    values: Vec<f64>,
}

impl FactorTable {
    pub fn new(
        id: impl Into<String>,
        scope: Vec<String>,
        values: Vec<f64>,
    ) -> Result<Self, EpistemicError> {
        let id = id.into();
        let mut seen = BTreeSet::new();
        for variable in &scope {
            if !seen.insert(variable.clone()) {
                return Err(EpistemicError::RepeatedVariableInScope {
                    factor: id,
                    variable: variable.clone(),
                });
            }
        }
        let want = 1usize << scope.len();
        if values.len() != want {
            let arity = scope.len();
            return Err(EpistemicError::FactorTableShape {
                factor: id,
                scope,
                got: values.len(),
                want,
                arity,
            });
        }
        for (index, value) in values.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(EpistemicError::InadmissiblePotential {
                    factor: id,
                    index,
                    value: *value,
                });
            }
        }
        Ok(FactorTable { id, scope, values })
    }

    /// Value at an assignment given as a map from variable name to bit.
    pub fn at(&self, assignment: &BTreeMap<String, u8>) -> f64 {
        let mut index = 0usize;
        for (position, variable) in self.scope.iter().enumerate() {
            if assignment.get(variable).copied().unwrap_or(0) == 1 {
                index |= 1 << position;
            }
        }
        self.values[index]
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }
}

/// A set of factors over binary variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactorGraph {
    factors: Vec<FactorTable>,
}

impl FactorGraph {
    pub fn new(factors: Vec<FactorTable>) -> Result<Self, EpistemicError> {
        let ids: Vec<String> = factors.iter().map(|f| f.id.clone()).collect();
        crate::unique(&ids, "factor graph")?;
        Ok(FactorGraph { factors })
    }

    pub fn factors(&self) -> &[FactorTable] {
        &self.factors
    }

    pub fn factor(&self, id: &str) -> Result<&FactorTable, EpistemicError> {
        self.factors
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| EpistemicError::UnknownIdentifier {
                collection: "factor graph".to_string(),
                id: id.to_string(),
            })
    }

    /// Every variable mentioned by any factor, in sorted order.
    pub fn variables(&self) -> Vec<String> {
        let set: BTreeSet<String> = self
            .factors
            .iter()
            .flat_map(|f| f.scope.iter().cloned())
            .collect();
        set.into_iter().collect()
    }
}

/// An assignment of factors to agents. Each factor belongs to exactly one agent.
///
/// Exclusivity is enforced rather than documented: a factor held by two agents is multiplied into
/// the product twice, and every subsequent number is wrong by that factor with nothing to indicate
/// it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Partition {
    assignment: BTreeMap<String, BTreeSet<String>>,
}

impl Partition {
    pub fn new(
        graph: &FactorGraph,
        assignment: BTreeMap<String, BTreeSet<String>>,
    ) -> Result<Self, EpistemicError> {
        let mut claimed: BTreeMap<String, String> = BTreeMap::new();
        for (agent, factors) in &assignment {
            if factors.is_empty() {
                return Err(EpistemicError::EmptyAgent {
                    agent: agent.clone(),
                });
            }
            for factor in factors {
                graph.factor(factor)?;
                if let Some(previous) = claimed.insert(factor.clone(), agent.clone()) {
                    return Err(EpistemicError::FactorAssignedTwice {
                        agent: previous,
                        factor: factor.clone(),
                    });
                }
            }
        }
        Ok(Partition { assignment })
    }

    pub fn agents(&self) -> Vec<String> {
        self.assignment.keys().cloned().collect()
    }

    pub fn factors_of(&self, agent: &str) -> Result<&BTreeSet<String>, EpistemicError> {
        self.assignment
            .get(agent)
            .ok_or_else(|| EpistemicError::UnknownIdentifier {
                collection: "partition".to_string(),
                id: agent.to_string(),
            })
    }

    /// Variables the agent's own factors mention.
    pub fn scope_of(
        &self,
        graph: &FactorGraph,
        agent: &str,
    ) -> Result<BTreeSet<String>, EpistemicError> {
        let mut scope = BTreeSet::new();
        for id in self.factors_of(agent)? {
            scope.extend(graph.factor(id)?.scope.iter().cloned());
        }
        Ok(scope)
    }
}

/// `S_ij = Vars(U_i) ∩ Vars(U_j)`, 43.29's separator.
pub fn separator(
    graph: &FactorGraph,
    partition: &Partition,
    left: &str,
    right: &str,
) -> Result<BTreeSet<String>, EpistemicError> {
    let a = partition.scope_of(graph, left)?;
    let b = partition.scope_of(graph, right)?;
    Ok(a.intersection(&b).cloned().collect())
}

/// Whether one collection pass is exact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Exactness {
    /// Acyclic agent graph with running intersection. 43.47 candidate C's premise holds.
    ExactOnTree,
    /// The agent graph has a cycle. One pass double-counts and the answer is labelled, not fixed.
    LoopyApproximate,
    /// Acyclic, but a variable appears in two agents not connected through agents that also hold
    /// it. Messages cannot carry the constraint and one pass is not exact.
    RunningIntersectionViolated,
}

/// The structural verdict on a partition, with the witness when it fails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureReport {
    pub exactness: Exactness,
    /// Agents on a cycle, when one was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle: Option<Vec<String>>,
    /// The variable whose agent set is disconnected, when running intersection fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disconnected_variable: Option<String>,
    /// Every non-empty separator, keyed by the agent pair joined with `|`.
    pub separators: BTreeMap<String, Vec<String>>,
}

impl StructureReport {
    pub fn is_exact(&self) -> bool {
        self.exactness == Exactness::ExactOnTree
    }

    /// The widest separator. 43.29's failure mode "large separators can eliminate the token
    /// advantage" is a statement about this number.
    pub fn max_separator_width(&self) -> usize {
        self.separators.values().map(Vec::len).max().unwrap_or(0)
    }
}

fn adjacency(
    graph: &FactorGraph,
    partition: &Partition,
) -> Result<BTreeMap<String, BTreeSet<String>>, EpistemicError> {
    let agents = partition.agents();
    let mut edges: BTreeMap<String, BTreeSet<String>> = agents
        .iter()
        .map(|a| (a.clone(), BTreeSet::new()))
        .collect();
    for (i, left) in agents.iter().enumerate() {
        for right in agents.iter().skip(i + 1) {
            if !separator(graph, partition, left, right)?.is_empty() {
                edges
                    .get_mut(left)
                    .ok_or_else(|| EpistemicError::UnknownIdentifier {
                        collection: "partition agents".into(),
                        id: left.clone(),
                    })?
                    .insert(right.clone());
                edges
                    .get_mut(right)
                    .ok_or_else(|| EpistemicError::UnknownIdentifier {
                        collection: "partition agents".into(),
                        id: right.clone(),
                    })?
                    .insert(left.clone());
            }
        }
    }
    Ok(edges)
}

fn find_cycle(edges: &BTreeMap<String, BTreeSet<String>>) -> Option<Vec<String>> {
    let mut visited: BTreeSet<String> = BTreeSet::new();
    for start in edges.keys() {
        if visited.contains(start) {
            continue;
        }
        let mut stack: Vec<(String, Option<String>, Vec<String>)> =
            vec![(start.clone(), None, vec![start.clone()])];
        let mut seen: BTreeSet<String> = BTreeSet::new();
        while let Some((node, parent, path)) = stack.pop() {
            if !seen.insert(node.clone()) {
                return Some(path);
            }
            visited.insert(node.clone());
            for neighbour in &edges[&node] {
                if Some(neighbour) == parent.as_ref() {
                    continue;
                }
                if seen.contains(neighbour) {
                    let mut witness = path.clone();
                    witness.push(neighbour.clone());
                    return Some(witness);
                }
                let mut next = path.clone();
                next.push(neighbour.clone());
                stack.push((neighbour.clone(), Some(node.clone()), next));
            }
        }
    }
    None
}

fn connected_within(
    edges: &BTreeMap<String, BTreeSet<String>>,
    members: &BTreeSet<String>,
) -> bool {
    let Some(start) = members.iter().next() else {
        return true;
    };
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::from(vec![start.clone()]);
    reached.insert(start.clone());
    while let Some(node) = queue.pop_front() {
        for neighbour in &edges[&node] {
            if members.contains(neighbour) && reached.insert(neighbour.clone()) {
                queue.push_back(neighbour.clone());
            }
        }
    }
    reached.len() == members.len()
}

/// Decides whether one collection pass over this partition is exact.
pub fn structure(
    graph: &FactorGraph,
    partition: &Partition,
) -> Result<StructureReport, EpistemicError> {
    let edges = adjacency(graph, partition)?;
    let agents = partition.agents();

    let mut separators = BTreeMap::new();
    for (i, left) in agents.iter().enumerate() {
        for right in agents.iter().skip(i + 1) {
            let shared = separator(graph, partition, left, right)?;
            if !shared.is_empty() {
                separators.insert(
                    format!("{left}|{right}"),
                    shared.into_iter().collect::<Vec<_>>(),
                );
            }
        }
    }

    if let Some(cycle) = find_cycle(&edges) {
        return Ok(StructureReport {
            exactness: Exactness::LoopyApproximate,
            cycle: Some(cycle),
            disconnected_variable: None,
            separators,
        });
    }

    for variable in graph.variables() {
        let mut holders = BTreeSet::new();
        for agent in &agents {
            if partition.scope_of(graph, agent)?.contains(&variable) {
                holders.insert(agent.clone());
            }
        }
        if !connected_within(&edges, &holders) {
            return Ok(StructureReport {
                exactness: Exactness::RunningIntersectionViolated,
                cycle: None,
                disconnected_variable: Some(variable),
                separators,
            });
        }
    }

    Ok(StructureReport {
        exactness: Exactness::ExactOnTree,
        cycle: None,
        disconnected_variable: None,
        separators,
    })
}

/// A local inconsistency that will not compress into a normal message.
///
/// 43.29's runtime contract requires this as a first-class alternative to a message, and its
/// failure-mode list says "one agent's obstruction cannot be voted away without adjudication". It
/// is a separate type from a message for exactly that reason: a struct with an optional
/// `obstruction: Option<..>` field alongside real values invites a consumer to read the values and
/// ignore the field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Obstruction {
    pub agent: String,
    pub to: String,
    /// What went wrong, as a machine-readable kind.
    pub kind: ObstructionKind,
    pub detail: String,
    /// The factors the agent held. Adjudication needs to know what it was reasoning from.
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObstructionKind {
    /// The agent's local factors are jointly unsatisfiable: every assignment has potential zero.
    /// There is no message that says this — a table of zeros would be read as "no evidence".
    LocallyInconsistent,
    /// The local assignment space is above [`MAX_LOCAL_ASSIGNMENTS`].
    LocalSpaceTooLarge,
}

/// A typed separator message.
///
/// `values` is a table over the separator variables in sorted order, same indexing convention as
/// [`FactorTable`]. `residual` is the mass this message discards by marginalising, which is what a
/// convergence monitor would watch; it is reported and not acted on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeparatorMessage {
    pub from: String,
    pub to: String,
    pub separator: Vec<String>,
    values: Vec<f64>,
    /// Factor ids the content was derived from. 43.29: "messages preserve source and scope".
    pub provenance: Vec<String>,
    /// Total mass of the local product, before marginalisation.
    pub residual: f64,
    /// Content address of everything above. Two agents holding messages with the same key hold
    /// the same message, which is what makes a fusion replayable.
    pub validity_key: String,
}

impl SeparatorMessage {
    /// Builds a message, refusing any scope that is not exactly the separator.
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        allowed_separator: &BTreeSet<String>,
        scope: Vec<String>,
        values: Vec<f64>,
        provenance: Vec<String>,
        residual: f64,
    ) -> Result<Self, EpistemicError> {
        let from = from.into();
        let to = to.into();
        for variable in &scope {
            if !allowed_separator.contains(variable) {
                return Err(EpistemicError::VariableOutsideSeparator {
                    from,
                    to,
                    variable: variable.clone(),
                    separator: allowed_separator.iter().cloned().collect(),
                });
            }
        }
        let want = 1usize << scope.len();
        if values.len() != want {
            return Err(EpistemicError::FactorTableShape {
                factor: format!("{from}->{to}"),
                scope,
                got: values.len(),
                want,
                arity: 0,
            });
        }
        let key = bioprism_ids::sha256_hex_of_value(&json!({
            "from": from,
            "to": to,
            "separator": scope,
            "values": values,
            "provenance": provenance,
        }))
        .map_err(|e| EpistemicError::QueryRejected {
            schema: "separator-message".to_string(),
            detail: e.to_string(),
        })?;
        Ok(SeparatorMessage {
            from,
            to,
            separator: scope,
            values,
            provenance,
            residual,
            validity_key: key,
        })
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Value at an assignment restricted to the separator.
    pub fn at(&self, assignment: &BTreeMap<String, u8>) -> f64 {
        let mut index = 0usize;
        for (position, variable) in self.separator.iter().enumerate() {
            if assignment.get(variable).copied().unwrap_or(0) == 1 {
                index |= 1 << position;
            }
        }
        self.values[index]
    }

    /// Bytes this message occupies on the wire, as canonical JSON.
    pub fn wire_bytes(&self) -> usize {
        serde_json::to_string(self).map(|s| s.len()).unwrap_or(0)
    }
}

/// Either a message or the obstruction that replaced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum LocalOutcome {
    Message(Box<SeparatorMessage>),
    Obstruction(Box<Obstruction>),
}

fn assignments(variables: &[String]) -> Vec<BTreeMap<String, u8>> {
    let total = 1usize << variables.len();
    (0..total)
        .map(|mask| {
            variables
                .iter()
                .enumerate()
                .map(|(position, name)| (name.clone(), ((mask >> position) & 1) as u8))
                .collect()
        })
        .collect()
}

/// One agent's message to a neighbour, given messages already received from its other neighbours.
pub fn local_message(
    graph: &FactorGraph,
    partition: &Partition,
    from: &str,
    to: &str,
    incoming: &[SeparatorMessage],
) -> Result<LocalOutcome, EpistemicError> {
    let scope: Vec<String> = partition.scope_of(graph, from)?.into_iter().collect();
    let provenance: Vec<String> = partition.factors_of(from)?.iter().cloned().collect();
    if scope.len() >= 20 || (1usize << scope.len()) > MAX_LOCAL_ASSIGNMENTS {
        return Ok(LocalOutcome::Obstruction(Box::new(Obstruction {
            agent: from.to_string(),
            to: to.to_string(),
            kind: ObstructionKind::LocalSpaceTooLarge,
            detail: format!(
                "agent holds {} variables, above the cap of {MAX_LOCAL_ASSIGNMENTS} assignments",
                scope.len()
            ),
            provenance,
        })));
    }

    let shared = separator(graph, partition, from, to)?;
    let shared_vec: Vec<String> = shared.iter().cloned().collect();
    let mut values = vec![0.0f64; 1usize << shared_vec.len()];
    let mut residual = 0.0f64;

    for assignment in assignments(&scope) {
        let mut product = 1.0f64;
        for id in &provenance {
            product *= graph.factor(id)?.at(&assignment);
        }
        for message in incoming {
            product *= message.at(&assignment);
        }
        residual += product;
        let mut index = 0usize;
        for (position, variable) in shared_vec.iter().enumerate() {
            if assignment.get(variable).copied().unwrap_or(0) == 1 {
                index |= 1 << position;
            }
        }
        values[index] += product;
    }

    if residual <= 0.0 {
        return Ok(LocalOutcome::Obstruction(Box::new(Obstruction {
            agent: from.to_string(),
            to: to.to_string(),
            kind: ObstructionKind::LocallyInconsistent,
            detail: "every local assignment has potential zero; a table of zeros would be read \
                     downstream as an absence of evidence rather than a contradiction"
                .to_string(),
            provenance,
        })));
    }

    Ok(LocalOutcome::Message(Box::new(SeparatorMessage::new(
        from, to, &shared, shared_vec, values, provenance, residual,
    )?)))
}

/// The result of one collection pass to a root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    /// Unnormalised marginal over `query`, in the same table convention.
    pub marginal: Vec<f64>,
    pub query: Vec<String>,
    pub exactness: Exactness,
    pub messages: Vec<SeparatorMessage>,
    /// Obstructions raised instead of messages. A non-empty list means the marginal is partial.
    pub obstructions: Vec<Obstruction>,
}

impl Collection {
    /// Total bytes of separator traffic.
    pub fn message_bytes(&self) -> usize {
        self.messages.iter().map(SeparatorMessage::wire_bytes).sum()
    }
}

/// Runs one collection pass toward `root` and marginalises to `query`.
///
/// `query` must be inside the root's local scope: a variable the root does not hold cannot be
/// marginalised to at the root, and silently widening the root's scope to accommodate one would
/// change the partition being measured.
pub fn collect(
    graph: &FactorGraph,
    partition: &Partition,
    root: &str,
    query: &[String],
) -> Result<Collection, EpistemicError> {
    let report = structure(graph, partition)?;
    let edges = adjacency(graph, partition)?;
    let root_scope = partition.scope_of(graph, root)?;
    for variable in query {
        if !root_scope.contains(variable) {
            return Err(EpistemicError::UnknownIdentifier {
                collection: format!("local scope of agent {root:?}"),
                id: variable.clone(),
            });
        }
    }

    let mut order: Vec<(String, Option<String>)> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack = vec![(root.to_string(), None::<String>)];
    while let Some((node, parent)) = stack.pop() {
        if !seen.insert(node.clone()) {
            continue;
        }
        order.push((node.clone(), parent.clone()));
        for neighbour in &edges[&node] {
            if Some(neighbour) != parent.as_ref() && !seen.contains(neighbour) {
                stack.push((neighbour.clone(), Some(node.clone())));
            }
        }
    }

    let mut inbox: BTreeMap<String, Vec<SeparatorMessage>> = BTreeMap::new();
    let mut messages: Vec<SeparatorMessage> = Vec::new();
    let mut obstructions: Vec<Obstruction> = Vec::new();

    for (node, parent) in order.iter().rev() {
        let Some(parent) = parent else {
            continue;
        };
        let incoming = inbox.get(node).cloned().unwrap_or_default();
        match local_message(graph, partition, node, parent, &incoming)? {
            LocalOutcome::Message(message) => {
                inbox
                    .entry(parent.clone())
                    .or_default()
                    .push(*message.clone());
                messages.push(*message);
            }
            LocalOutcome::Obstruction(obstruction) => obstructions.push(*obstruction),
        }
    }

    let scope: Vec<String> = root_scope.into_iter().collect();
    let root_factors: Vec<String> = partition.factors_of(root)?.iter().cloned().collect();
    let incoming = inbox.get(root).cloned().unwrap_or_default();
    let mut marginal = vec![0.0f64; 1usize << query.len()];
    for assignment in assignments(&scope) {
        let mut product = 1.0f64;
        for id in &root_factors {
            product *= graph.factor(id)?.at(&assignment);
        }
        for message in &incoming {
            product *= message.at(&assignment);
        }
        let mut index = 0usize;
        for (position, variable) in query.iter().enumerate() {
            if assignment.get(variable).copied().unwrap_or(0) == 1 {
                index |= 1 << position;
            }
        }
        marginal[index] += product;
    }

    Ok(Collection {
        marginal,
        query: query.to_vec(),
        exactness: report.exactness,
        messages,
        obstructions,
    })
}

/// The same marginal computed by enumerating every joint assignment.
///
/// Ground truth for [`collect`]. Uses a different code path on purpose — no partition, no
/// messages, no marginalisation order — so a bug in the message machinery cannot hide by being
/// present on both sides, which is the mistake `bioprism-influence` names in its brute-force
/// module.
pub fn centralised(graph: &FactorGraph, query: &[String]) -> Result<Vec<f64>, EpistemicError> {
    let variables = graph.variables();
    if variables.len() >= 20 || (1usize << variables.len()) > MAX_JOINT_ASSIGNMENTS {
        return Err(EpistemicError::ExhaustiveCapExceeded {
            ground: variables.len(),
            needed: 1u64 << variables.len().min(63),
            cap: MAX_JOINT_ASSIGNMENTS as u64,
        });
    }
    let mut marginal = vec![0.0f64; 1usize << query.len()];
    for assignment in assignments(&variables) {
        let mut product = 1.0f64;
        for factor in graph.factors() {
            product *= factor.at(&assignment);
        }
        let mut index = 0usize;
        for (position, variable) in query.iter().enumerate() {
            if assignment.get(variable).copied().unwrap_or(0) == 1 {
                index |= 1 << position;
            }
        }
        marginal[index] += product;
    }
    Ok(marginal)
}

/// What the protocol cost, against sharing everything.
///
/// 43.29's evaluation program leads with "total tokens and bytes versus transcript-sharing teams".
/// The transcript baseline here is every agent's full factor tables serialised once per *other*
/// agent, which is what "every participant receives the same large context" means concretely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransportCost {
    pub message_bytes: usize,
    pub transcript_bytes: usize,
    pub messages: usize,
    pub agents: usize,
    /// `message_bytes / transcript_bytes`. Below one means the separator protocol paid off.
    pub ratio: f64,
}

/// Measures separator traffic against the transcript-sharing baseline.
pub fn transport_cost(
    graph: &FactorGraph,
    partition: &Partition,
    collection: &Collection,
) -> Result<TransportCost, EpistemicError> {
    let agents = partition.agents();
    let mut per_agent = 0usize;
    for agent in &agents {
        for id in partition.factors_of(agent)? {
            per_agent += serde_json::to_string(graph.factor(id)?)
                .map(|s| s.len())
                .unwrap_or(0);
        }
    }
    let others = agents.len().saturating_sub(1);
    let transcript = per_agent * others.max(1);
    let message_bytes = collection.message_bytes();
    Ok(TransportCost {
        message_bytes,
        transcript_bytes: transcript,
        messages: collection.messages.len(),
        agents: agents.len(),
        ratio: if transcript == 0 {
            1.0
        } else {
            message_bytes as f64 / transcript as f64
        },
    })
}
