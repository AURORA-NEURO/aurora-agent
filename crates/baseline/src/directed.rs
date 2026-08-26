//! Directed dependency walk.
//!
//! The second baseline `docs/FINDINGS.md` named as missing: a walk over the **directed** factor
//! edges rather than the undirected incidence projection [`crate::incidence`] uses. From each
//! needed variable it steps to the factors that *output* it, then to those factors' input
//! variables, transitively, to a declared depth; unbounded depth is the full backward slice.
//! Direction is the entire difference from [`crate::incidence::KHopIncidence`]: a distractor
//! factor that merely *consumes* a hub variable is never entered, because consuming a variable
//! does not make a factor part of anything's dependency history.
//!
//! The protected closure is taken first and unconditionally, exactly as FIBER takes it: 43.13
//! declares the closure mandatory *before* any relevance step, so a baseline claiming to satisfy
//! the same contract computes it the same way rather than being scored down for not knowing about
//! it. The slice and the closure are reported separately in the selection notes so a reader can
//! see which of the two did the work.
//!
//! # The measured result is against FIBER, and it ships
//!
//! FINDINGS.md predicted this baseline "would recover much of what backward slicing does". The
//! measurement is stronger: on the reference world **and** on the discriminating world the
//! unbounded walk selects **exactly the eleven facts FIBER compiles** — the identical set, sound,
//! with full protected closure. Equal engineering leaves the two indistinguishable on every world
//! in the shipped structural sweep; see `tests/directed_walk.rs` and `docs/FINDINGS.md`. What the
//! walk does not reproduce is the rest of the compiler's contract — the temporal cut, the policy
//! screen and the certificate stating what was omitted and why — none of which these worlds'
//! verdicts exercise.

use crate::index::PanelIndex;
use crate::strategy::{ContextStrategy, Selection};
use bioprism_fiber::Query;
use bioprism_world::World;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub struct DirectedDependencyWalk {
    /// Maximum factor-hops backward from a query target. `None` is the full backward slice.
    pub depth: Option<usize>,
}

impl DirectedDependencyWalk {
    /// The full backward slice, the setting entered in the panels.
    pub fn unbounded() -> Self {
        DirectedDependencyWalk { depth: None }
    }

    /// The raw backward slice, without the protected closure.
    ///
    /// Public so a test can state which facts the *walk* found, as distinct from the facts the
    /// mandatory closure would have contributed anyway — without it, "the walk recovers what
    /// backward slicing does" would be unfalsifiable, because the closure alone is
    /// decision-sufficient on the shipped worlds.
    pub fn slice(&self, world: &World, query: &Query) -> BTreeSet<String> {
        let mut producers: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (position, factor) in world.factors.iter().enumerate() {
            for output in &factor.outputs {
                producers.entry(output.as_str()).or_default().push(position);
            }
        }

        let mut needed: BTreeSet<String> = query
            .targets
            .iter()
            .map(|target| target.as_str().to_string())
            .collect();
        let mut frontier: VecDeque<(String, usize)> = needed
            .iter()
            .cloned()
            .map(|variable| (variable, 0))
            .collect();

        while let Some((variable, hops)) = frontier.pop_front() {
            if self.depth.is_some_and(|limit| hops >= limit) {
                continue;
            }
            let Some(positions) = producers.get(variable.as_str()) else {
                continue;
            };
            for position in positions {
                for input in &world.factors[*position].inputs {
                    if needed.insert(input.as_str().to_string()) {
                        frontier.push_back((input.as_str().to_string(), hops + 1));
                    }
                }
            }
        }

        world
            .facts
            .iter()
            .filter(|fact| needed.contains(fact.provides.as_str()))
            .map(|fact| fact.id.as_str().to_string())
            .collect()
    }
}

impl ContextStrategy for DirectedDependencyWalk {
    fn name(&self) -> String {
        match self.depth {
            None => "directed-walk-full".into(),
            Some(depth) => format!("directed-walk-{depth}"),
        }
    }

    fn method(&self) -> String {
        let reach = match self.depth {
            None => "unbounded (the full backward slice)".to_string(),
            Some(depth) => format!("to {depth} factor-hop(s)"),
        };
        format!(
            "protected closure first (mandatory, as 43.13 orders it), then a walk of the directed \
             factor graph backward from the query targets — needed variable to the factors that \
             output it, to their input variables, transitively — {reach}; facts providing any \
             needed variable are selected"
        )
    }

    fn select_indexed(&self, index: &PanelIndex<'_>) -> Selection {
        let (world, query) = (index.world(), index.query());
        let closure: BTreeSet<String> = world
            .facts
            .iter()
            .filter(|fact| fact.has_any_tag(&query.protected_tags))
            .map(|fact| fact.id.as_str().to_string())
            .collect();
        let slice = self.slice(world, query);

        let sliced_beyond_closure = slice.difference(&closure).count();
        let note = format!(
            "protected closure contributed {} fact(s), the backward slice {} (of which {} beyond \
             the closure); edges are directed, so factors that only consume a hub are never entered",
            closure.len(),
            slice.len(),
            sliced_beyond_closure
        );

        Selection::new(closure.union(&slice).cloned().collect()).noting(note)
    }
}
