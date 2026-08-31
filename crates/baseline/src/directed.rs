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
//!
//! # The tie is entailed, not observed
//!
//! That 36-of-36 tie was reported as a measurement. It is a theorem, and stating it as one is what
//! makes the benchmark's limit visible.
//!
//! [`DirectedDependencyWalk`] selects `closure ∪ slice`. FIBER selects
//! `(closure ∪ slice') ∖ withheld_by_policy ∖ inaccessible_at_cut`, where `slice'` is this walk's
//! `slice` with at most one provider kept per needed variable
//! (`bioprism_world::WorldSource::fact_providing` returns the last in document order). Both
//! fixpoints are the same: [`crate::directed::DirectedDependencyWalk::slice`] and
//! [`bioprism_fiber::backward_slice`] step needed variable → producing factor → factor input over
//! the same directed edges, each expanding a factor at most once, so they agree on
//! `needed_variables` exactly. Union is monotone and set difference only removes, so
//!
//! ```text
//! fiber(world, query) ⊆ directed-walk-full(world, query)     for every world and query
//! ```
//!
//! holds unconditionally, and equality holds **exactly** when all three of the following are true.
//! Each is an escape hatch — a knob that, if moved, separates the two strategies:
//!
//! 1. **No policy withholding lands on the selection.** Fired by a fact binding the `policy` scope
//!    dimension to a clause the query does not accept — `WorldSpec::policy_restricted`.
//! 2. **No selected fact is temporally inaccessible at the decision time.** Fired by an event
//!    releasing a needed variable after the cut — `WorldSpec::external_confirmation`, or any
//!    `events × decision_time` pair that puts a release on the far side of the cut.
//! 3. **No needed variable has a shadowed provider outside the closure.** Fired by two facts
//!    providing one variable, where FIBER keeps the document-order winner and the walk keeps both.
//!
//! Two further conditions are refusals rather than divergences, and are named so nobody reads an
//! empty selection as a small one: a FIBER compile that errors (a policy conflict, a withheld
//! protected fact, an exceeded budget) yields an empty selection, which is a subset of everything
//! and equal to nothing; and the screen can reject a malformed policy requirement carried by a
//! fact only the walk holds.
//!
//! `crates/baseline/tests/selection_equivalence.rs` asserts the subset relation and the equality
//! condition over the sweep's own specs and over the two presets that fire the hatches. The
//! consequence for the shipped sweep is stated in `docs/FINDINGS.md` §7: the grid varies
//! attachment, relay depth, tag style and distractor count, none of which appear above, so the
//! sweep could not have separated these two strategies whatever it measured.
//!
//! # The counter-baselines
//!
//! [`ScreenedDependencyWalk`] closes the gap the entailment names by handing the walk the passes
//! it was never given. A comparison in which only one competitor carries the temporal cut and the
//! policy screen measures the passes, not the compiler, and reporting it as a win for the compiler
//! would be `compare_baselines.py`'s own mistake one level up.

use crate::index::PanelIndex;
use crate::strategy::{ContextStrategy, Selection};
use bioprism_fiber::{policy, temporal_cut, PolicyEnvelope, Query};
use bioprism_world::{Fact, World};
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
        let closure = protected_closure(world, query);
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

/// Facts carrying a protected tag, computed the way 43.13 orders it: before any relevance step.
///
/// Shared by every walk in this module so that no counter-baseline can accidentally be handed a
/// different mandatory closure from the one the naive walk takes, which would make the panel
/// unequal in the direction the module documentation warns about.
fn protected_closure(world: &World, query: &Query) -> BTreeSet<String> {
    world
        .facts
        .iter()
        .filter(|fact| fact.has_any_tag(&query.protected_tags))
        .map(|fact| fact.id.as_str().to_string())
        .collect()
}

/// Which of FIBER's subtractive passes a walk carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineeredPasses {
    /// The temporal cut of 43.09 only: evidence an event had not released by the decision time is
    /// dropped. Isolates the release schedule from the access-control question.
    TemporalCut,
    /// The policy screen of 43.33 only: evidence whose `policy` scope names a clause the query did
    /// not accept is dropped. Isolates access control from the release schedule.
    PolicyScreen,
    /// Both, in FIBER's own order — screen first over the materialised candidates, then the cut.
    ///
    /// This is FIBER's selection algebra with no certificate attached, and it is the baseline the
    /// honesty argument turns on: if it ties FIBER, the compiler's measurable selection advantage
    /// is exactly the two passes and not the compiler.
    Both,
}

impl EngineeredPasses {
    fn screens_policy(self) -> bool {
        matches!(
            self,
            EngineeredPasses::PolicyScreen | EngineeredPasses::Both
        )
    }

    fn applies_cut(self) -> bool {
        matches!(self, EngineeredPasses::TemporalCut | EngineeredPasses::Both)
    }
}

/// The full backward walk, carrying FIBER's subtractive passes.
///
/// The panel's obligation under 43.38 is that "no baseline receives less data access or weaker
/// tools", and [`DirectedDependencyWalk`] receives strictly weaker tools than the compiler it is
/// compared against: it is handed the mandatory closure and the directed slice and then denied the
/// two passes that are the only reason FIBER's selection can differ from it at all. Any world on
/// which those passes fire scores the walk down for equipment it was never issued.
///
/// Every pass here is the *same code* FIBER runs — [`bioprism_fiber::temporal_cut`] and
/// [`bioprism_fiber::policy::screen`] — rather than a re-implementation, so a divergence between
/// this walk and the compiler cannot be an artefact of two authors reading one specification
/// differently.
///
/// What this still does not carry, and what therefore remains genuinely FIBER-only: the Context
/// Certificate, its influence manifest, the omission classes that name *why* each dropped fact was
/// dropped, and the refinement frontier. Those are not selections and no admissibility column can
/// see them.
pub struct ScreenedDependencyWalk {
    pub passes: EngineeredPasses,
}

impl ScreenedDependencyWalk {
    /// The walk plus the temporal cut.
    pub fn cut() -> Self {
        ScreenedDependencyWalk {
            passes: EngineeredPasses::TemporalCut,
        }
    }

    /// The walk plus the policy screen.
    pub fn screened() -> Self {
        ScreenedDependencyWalk {
            passes: EngineeredPasses::PolicyScreen,
        }
    }

    /// The walk plus both passes: FIBER's selection algebra, no certificate.
    pub fn compiled() -> Self {
        ScreenedDependencyWalk {
            passes: EngineeredPasses::Both,
        }
    }
}

impl ContextStrategy for ScreenedDependencyWalk {
    fn name(&self) -> String {
        match self.passes {
            EngineeredPasses::TemporalCut => "directed-walk-cut".into(),
            EngineeredPasses::PolicyScreen => "directed-walk-screened".into(),
            EngineeredPasses::Both => "directed-walk-compiled".into(),
        }
    }

    fn method(&self) -> String {
        let carried = match self.passes {
            EngineeredPasses::TemporalCut => "then the temporal cut of 43.09",
            EngineeredPasses::PolicyScreen => "then the policy screen of 43.33",
            EngineeredPasses::Both => {
                "then the policy screen of 43.33 and the temporal cut of 43.09"
            }
        };
        format!(
            "protected closure first (mandatory, as 43.13 orders it), then the unbounded backward \
             walk of the directed factor graph, {carried} — the compiler's own pass code, run over \
             the walk's selection; no certificate is produced"
        )
    }

    fn select_indexed(&self, index: &PanelIndex<'_>) -> Selection {
        let (world, query) = (index.world(), index.query());
        let closure = protected_closure(world, query);
        let slice = DirectedDependencyWalk::unbounded().slice(world, query);
        let mut selected: BTreeSet<String> = closure.union(&slice).cloned().collect();
        let walked = selected.len();
        let mut notes = Vec::new();

        if self.passes.screens_policy() {
            let envelope = match PolicyEnvelope::resolve(world, query) {
                Ok(envelope) => envelope,
                Err(violation) => return refused(format!("policy envelope refused: {violation}")),
            };
            let candidates: BTreeMap<String, Fact> = selected
                .iter()
                .filter_map(|id| world.fact(id).map(|fact| (id.clone(), fact.clone())))
                .collect();
            let screen = match policy::screen(&envelope, &candidates, &closure) {
                Ok(screen) => screen,
                Err(violation) => return refused(format!("policy screen refused: {violation}")),
            };
            let withheld = screen.withheld_ids();
            for id in &withheld {
                selected.remove(id);
            }
            notes.push(format!(
                "policy screen: {} candidate(s) declared a requirement, {} withheld",
                screen.requirements_seen(),
                withheld.len()
            ));
        }

        if self.passes.applies_cut() {
            let cut = temporal_cut(world, query.decision_time);
            let inaccessible: Vec<String> = selected
                .iter()
                .filter(|id| {
                    world
                        .fact(id)
                        .is_some_and(|fact| !cut.is_accessible(fact.provides.as_str()))
                })
                .cloned()
                .collect();
            for id in &inaccessible {
                selected.remove(id);
            }
            notes.push(format!(
                "temporal cut: {} fact(s) withheld at the decision time",
                inaccessible.len()
            ));
        }

        notes.insert(
            0,
            format!(
                "the walk selected {walked} fact(s) before the carried pass(es), {} after",
                selected.len()
            ),
        );

        notes
            .into_iter()
            .fold(Selection::new(selected), |selection, note| {
                selection.noting(note)
            })
    }
}

/// A refusal, reported the way [`crate::strategy::FiberCompiled`] reports a failed compile.
///
/// An empty selection is not a compact one, and the note is the only place a reader learns which
/// of the two it was. `crate::compare` will judge the empty selection and mark the row unsound,
/// which is the same treatment FIBER's own refusal receives — matching them is the point.
fn refused(reason: String) -> Selection {
    Selection::new(BTreeSet::new()).noting(format!("{reason}; selection is empty"))
}
