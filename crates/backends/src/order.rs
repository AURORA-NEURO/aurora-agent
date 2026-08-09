//! Elimination order and induced width.
//!
//! Blueprint 43.18: "variable order can dominate relational joins, graphical-model inference,
//! tensor contraction, and decision diagrams", and the width of the chosen order — not the size of
//! the world — is the complexity predictor. This module computes the order and measures the width
//! it induces.
//!
//! ## What width means here
//!
//! Build the primal graph of the region: one vertex per variable, and a clique over the scope of
//! every factor. Play the elimination game — remove each variable in turn, connecting the
//! neighbours it still has — and the *induced width* of an order is the largest such neighbourhood
//! encountered. A variable eliminated with `k` remaining neighbours produces an intermediate
//! factor over exactly those `k`, so `max_v ∏ card(neighbours)` is the peak memory and
//! `∑_v ∏ card(clique)` the work. Width is a structural quantity; it becomes a cost only once
//! multiplied through the cardinalities, which is why 43.18 asks for both.
//!
//! ## Heuristic width is an upper bound, not treewidth
//!
//! Finding the order of minimum induced width is NP-hard, so [`OrderStrategy::MinDegree`] and
//! [`OrderStrategy::MinFill`] return orders whose width upper-bounds the region's treewidth.
//! 43.48 forbids presenting heuristic success as worst-case tractability, so every
//! [`EliminationOrder`] states its [`Bound`] and only [`OrderStrategy::ExactMinimumWidth`] — run by
//! subset dynamic programming, and only on regions small enough for it — reports `Exact`.

use crate::region::QueryRegion;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Largest number of eliminated variables for which the exact optimum is computed.
///
/// The subset dynamic program visits `2^n` states, so this cap is the point at which exact search
/// stops being cheaper than the execution it is trying to plan — 43.48's "optimization overhead
/// can exceed savings for small queries", read from the other end.
pub const EXACT_ORDER_MAX_VARIABLES: usize = 14;

/// Which width metric a claim is about. 43.18: "width claims state the metric and assumptions."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidthMetric {
    /// Induced width of a specific elimination order over the region's primal graph.
    InducedWidth,
    /// The backend does not eliminate, so no width describes it.
    NotApplicable,
}

/// Whether a reported width is the true optimum or an upper bound on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bound {
    Exact,
    HeuristicUpperBound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStrategy {
    /// Eliminate the variable with the fewest current neighbours. Cheap, and blind to how many
    /// fill edges the elimination will create.
    MinDegree,
    /// Eliminate the variable whose elimination adds the fewest fill edges. More expensive per
    /// step and usually tighter.
    MinFill,
    /// Subset dynamic programming over eliminated sets. Exact, and only attempted when the number
    /// of eliminated variables is at most [`EXACT_ORDER_MAX_VARIABLES`]; otherwise it steps down
    /// to [`OrderStrategy::MinFill`] and says so in [`EliminationOrder::used`].
    ExactMinimumWidth,
}

impl OrderStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderStrategy::MinDegree => "min_degree",
            OrderStrategy::MinFill => "min_fill",
            OrderStrategy::ExactMinimumWidth => "exact_minimum_width",
        }
    }
}

/// The clique an eliminated variable forms at the moment it is removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EliminationClique {
    pub eliminated: String,
    /// The eliminated variable together with the neighbours it still had.
    pub clique: Vec<String>,
}

impl EliminationClique {
    pub fn width(&self) -> usize {
        self.clique.len().saturating_sub(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EliminationOrder {
    pub requested: OrderStrategy,
    /// What actually ran. Differs from `requested` when exact search was declined as too large.
    pub used: OrderStrategy,
    pub bound: Bound,
    /// Bound variables in elimination order.
    pub order: Vec<String>,
    /// Free variables, which are never eliminated.
    pub retained: Vec<String>,
    pub induced_width: usize,
    pub cliques: Vec<EliminationClique>,
}

/// The primal (moral) graph of a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimalGraph {
    names: Vec<String>,
    index: BTreeMap<String, usize>,
    adjacency: Vec<BTreeSet<usize>>,
}

impl PrimalGraph {
    /// One vertex per region variable; a clique over every factor scope.
    ///
    /// Every region variable becomes a vertex even if no factor mentions it. An isolated variable
    /// still has to be aggregated away, and dropping it here would understate the answer by a
    /// factor of its cardinality.
    pub fn of_region(region: &QueryRegion) -> Self {
        let names: Vec<String> = region.variables().map(str::to_string).collect();
        let index: BTreeMap<String, usize> = names
            .iter()
            .enumerate()
            .map(|(position, name)| (name.clone(), position))
            .collect();
        let mut adjacency = vec![BTreeSet::new(); names.len()];
        for factor in region.factors() {
            let scope: Vec<usize> = factor.scope().iter().map(|name| index[name]).collect();
            for left in &scope {
                for right in &scope {
                    if left != right {
                        adjacency[*left].insert(*right);
                    }
                }
            }
        }
        PrimalGraph {
            names,
            index,
            adjacency,
        }
    }

    pub fn variable_count(&self) -> usize {
        self.names.len()
    }

    pub fn edge_count(&self) -> usize {
        self.adjacency.iter().map(BTreeSet::len).sum::<usize>() / 2
    }

    pub fn degree(&self, variable: &str) -> Option<usize> {
        self.index
            .get(variable)
            .map(|position| self.adjacency[*position].len())
    }

    pub fn neighbours(&self, variable: &str) -> Option<Vec<&str>> {
        let position = *self.index.get(variable)?;
        Some(
            self.adjacency[position]
                .iter()
                .map(|other| self.names[*other].as_str())
                .collect(),
        )
    }

    /// The induced width of a given order, together with the clique each elimination forms.
    ///
    /// Variables absent from `order` — the query's free variables — stay in the graph forever, so
    /// they count towards every clique they touch and are never themselves a width contribution.
    pub fn induced_width_of(&self, order: &[String]) -> (usize, Vec<EliminationClique>) {
        let mut working = self.adjacency.clone();
        let mut alive: BTreeSet<usize> = (0..self.names.len()).collect();
        let mut width = 0usize;
        let mut cliques = Vec::with_capacity(order.len());

        for variable in order {
            let Some(position) = self.index.get(variable).copied() else {
                continue;
            };
            if !alive.remove(&position) {
                continue;
            }
            let neighbours: Vec<usize> = working[position]
                .iter()
                .copied()
                .filter(|other| alive.contains(other))
                .collect();
            width = width.max(neighbours.len());

            let mut clique: Vec<String> = neighbours
                .iter()
                .map(|other| self.names[*other].clone())
                .collect();
            clique.push(variable.clone());
            clique.sort();
            cliques.push(EliminationClique {
                eliminated: variable.clone(),
                clique,
            });

            for left in &neighbours {
                for right in &neighbours {
                    if left != right {
                        working[*left].insert(*right);
                    }
                }
                working[*left].remove(&position);
            }
        }

        (width, cliques)
    }
}

/// Computes an elimination order for the region's bound variables.
pub fn elimination_order(region: &QueryRegion, strategy: OrderStrategy) -> EliminationOrder {
    let graph = PrimalGraph::of_region(region);
    let bound = region.bound_variables();
    let retained: BTreeSet<String> = region.free_variables().clone();

    let (order, used, bound_kind) = match strategy {
        OrderStrategy::MinDegree => (greedy_order(&graph, &bound, Cost::Degree), strategy, Bound::HeuristicUpperBound),
        OrderStrategy::MinFill => (greedy_order(&graph, &bound, Cost::Fill), strategy, Bound::HeuristicUpperBound),
        OrderStrategy::ExactMinimumWidth => match exact_order(&graph, &bound) {
            Some(order) => (order, strategy, Bound::Exact),
            None => (
                greedy_order(&graph, &bound, Cost::Fill),
                OrderStrategy::MinFill,
                Bound::HeuristicUpperBound,
            ),
        },
    };

    let (induced_width, cliques) = graph.induced_width_of(&order);

    EliminationOrder {
        requested: strategy,
        used,
        bound: bound_kind,
        order,
        retained: retained.into_iter().collect(),
        induced_width,
        cliques,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cost {
    Degree,
    Fill,
}

/// Greedy elimination, ties broken by variable name so the order is reproducible.
///
/// 43.18's runtime contract requires a plan to be replayable, which a hash-ordered tie-break would
/// not be.
fn greedy_order(graph: &PrimalGraph, bound: &BTreeSet<String>, cost: Cost) -> Vec<String> {
    let mut working = graph.adjacency.clone();
    let mut remaining: BTreeSet<usize> = bound
        .iter()
        .filter_map(|name| graph.index.get(name).copied())
        .collect();
    let mut order = Vec::with_capacity(remaining.len());

    while !remaining.is_empty() {
        let chosen = remaining
            .iter()
            .copied()
            .min_by_key(|position| {
                let score = match cost {
                    Cost::Degree => working[*position].len(),
                    Cost::Fill => fill_in_count(&working, *position),
                };
                (score, graph.names[*position].clone())
            })
            .expect("remaining is non-empty");

        let neighbours: Vec<usize> = working[chosen].iter().copied().collect();
        for left in &neighbours {
            for right in &neighbours {
                if left != right {
                    working[*left].insert(*right);
                }
            }
            working[*left].remove(&chosen);
        }
        working[chosen].clear();

        remaining.remove(&chosen);
        order.push(graph.names[chosen].clone());
    }

    order
}

fn fill_in_count(adjacency: &[BTreeSet<usize>], position: usize) -> usize {
    let neighbours: Vec<usize> = adjacency[position].iter().copied().collect();
    let mut missing = 0usize;
    for (offset, left) in neighbours.iter().enumerate() {
        for right in &neighbours[offset + 1..] {
            if !adjacency[*left].contains(right) {
                missing += 1;
            }
        }
    }
    missing
}

/// Exact minimum-induced-width order by dynamic programming over eliminated sets.
///
/// The fill-in graph after eliminating a set `S` depends only on `S` — two vertices are adjacent
/// iff some path between them runs entirely through `S` — so the state space is the power set of
/// the bound variables and the recursion
/// `w(S) = min_{v ∉ S} max(|N_{G_S}(v)|, w(S ∪ {v}))`
/// is exact. Returns `None` when the region is above [`EXACT_ORDER_MAX_VARIABLES`].
fn exact_order(graph: &PrimalGraph, bound: &BTreeSet<String>) -> Option<Vec<String>> {
    let positions: Vec<usize> = bound
        .iter()
        .filter_map(|name| graph.index.get(name).copied())
        .collect();
    if positions.len() > EXACT_ORDER_MAX_VARIABLES || positions.len() > 63 {
        return None;
    }

    let mut memo: BTreeMap<u64, (usize, Option<usize>)> = BTreeMap::new();
    let full: u64 = if positions.is_empty() {
        0
    } else {
        (1u64 << positions.len()) - 1
    };

    exact_step(graph, &positions, 0, full, &mut memo);

    let mut order = Vec::with_capacity(positions.len());
    let mut state = 0u64;
    while state != full {
        let (_, next) = *memo.get(&state)?;
        let slot = next?;
        order.push(graph.names[positions[slot]].clone());
        state |= 1u64 << slot;
    }
    Some(order)
}

fn exact_step(
    graph: &PrimalGraph,
    positions: &[usize],
    state: u64,
    full: u64,
    memo: &mut BTreeMap<u64, (usize, Option<usize>)>,
) -> usize {
    if state == full {
        return 0;
    }
    if let Some((width, _)) = memo.get(&state) {
        return *width;
    }

    let adjacency = fill_in_closure(graph, positions, state);
    let eliminated: BTreeSet<usize> = positions
        .iter()
        .enumerate()
        .filter(|(slot, _)| state & (1u64 << slot) != 0)
        .map(|(_, value)| *value)
        .collect();
    let mut best = usize::MAX;
    let mut best_slot = None;

    for (slot, position) in positions.iter().enumerate() {
        if state & (1u64 << slot) != 0 {
            continue;
        }
        let degree = adjacency[*position]
            .iter()
            .filter(|other| !eliminated.contains(other))
            .count();
        let tail = exact_step(graph, positions, state | (1u64 << slot), full, memo);
        let candidate = degree.max(tail);
        if candidate < best {
            best = candidate;
            best_slot = Some(slot);
        }
    }

    memo.insert(state, (best, best_slot));
    best
}

/// The graph induced by eliminating the variables selected in `state`.
fn fill_in_closure(graph: &PrimalGraph, positions: &[usize], state: u64) -> Vec<BTreeSet<usize>> {
    let mut working = graph.adjacency.clone();
    for (slot, position) in positions.iter().enumerate() {
        if state & (1u64 << slot) == 0 {
            continue;
        }
        let neighbours: Vec<usize> = working[*position].iter().copied().collect();
        for left in &neighbours {
            for right in &neighbours {
                if left != right {
                    working[*left].insert(*right);
                }
            }
        }
    }
    working
}
