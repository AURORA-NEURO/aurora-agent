//! The selection algebra relating FIBER to the directed walk, asserted rather than observed.
//!
//! `docs/FINDINGS.md` §6 reported `fiber` and `directed-walk-full` tied in all 36 cells of the
//! structural sweep and called the tie "overdetermined". It is stronger than that and weaker than
//! that at once: the tie is *entailed* by how the two strategies build a selection, so no cell of
//! that grid could have come out any other way — which means the 36-of-36 result is a fact about
//! the grid and not a measurement of either strategy.
//!
//! The entailment, stated as this file asserts it:
//!
//! ```text
//! walk   = closure ∪ walk_slice
//! fiber  = (closure ∪ fiber_slice) ∖ withheld_by_policy ∖ inaccessible_at_cut
//! ```
//!
//! Both slices are the same backward fixpoint over the same directed edges; they differ only in
//! that `fiber_slice` keeps one provider per needed variable — the document-order winner
//! `bioprism_world::WorldSource::fact_providing` returns — where `walk_slice` keeps every fact
//! providing a needed variable. So `fiber_slice ⊆ walk_slice`, union is monotone, and difference
//! only removes:
//!
//! * `fiber ⊆ walk` on **every** world and query, unconditionally; and
//! * `walk ∖ fiber` is **exactly** the union of three sets, each the output of one named escape
//!   hatch: shadowed providers outside the closure, the policy screen's withholdings, and the
//!   temporal cut's withholdings.
//!
//! The second form is the useful one: it turns "when do they differ?" from a question about
//! worlds into a question about which of three knobs a world moves. The default sweep grid moves
//! none of the three, which is asserted here as the reason the sweep cannot discriminate.
//!
//! A refused compile is the one shape outside the algebra and is tested separately: FIBER's
//! failure surfaces as an empty selection, which satisfies the subset relation vacuously and
//! carries no information about either strategy.

use bioprism_baseline::sweep::SweepGrid;
use bioprism_baseline::{
    ContextStrategy, DirectedDependencyWalk, FiberCompiled, ScreenedDependencyWalk,
};
use bioprism_fiber::{backward_slice, Query};
use bioprism_world::{World, WorldSource};
use bioprism_worldgen::{generate, PolicySpec, WorldSpec};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

const LOCAL_LAB_FACT: &str = "fact.local_lab";
const CENTRAL_LAB_FACT: &str = "fact.central_lab";

fn fixture(name: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "fiber-v0.1",
        name,
    ]
    .iter()
    .collect();
    serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture readable"))
        .expect("fixture is valid JSON")
}

fn reference() -> (World, Query) {
    (
        World::from_json(fixture("radiogenomic_world.json")).expect("the shipped world loads"),
        Query::from_json(fixture("leakage_query.json")).expect("the shipped query loads"),
    )
}

fn built(spec: &WorldSpec) -> (World, Query) {
    let generated = generate(spec);
    (
        World::from_json(generated.world).expect("generated world loads"),
        Query::from_json(generated.query).expect("generated query loads"),
    )
}

fn ids(facts: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    facts.into_iter().collect()
}

fn closure_of(world: &World, query: &Query) -> BTreeSet<String> {
    world
        .facts
        .iter()
        .filter(|fact| fact.has_any_tag(&query.protected_tags))
        .map(|fact| fact.id.as_str().to_string())
        .collect()
}

/// The three escape hatches, evaluated on one world.
///
/// Each field is read from the pass that owns it rather than inferred from the selections it is
/// being used to explain: `withheld_by_policy` and `inaccessible_at_cut` come off the compiler's
/// own trace and certificate, and `shadowed` is computed from
/// [`WorldSource::fact_providing`]'s document-order tiebreak. A derivation from the selection
/// difference would make the exactness claim circular.
struct Hatches {
    shadowed: BTreeSet<String>,
    withheld_by_policy: BTreeSet<String>,
    inaccessible_at_cut: BTreeSet<String>,
}

impl Hatches {
    fn none_fired(&self) -> bool {
        self.shadowed.is_empty()
            && self.withheld_by_policy.is_empty()
            && self.inaccessible_at_cut.is_empty()
    }

    fn union(&self) -> BTreeSet<String> {
        self.shadowed
            .union(&self.withheld_by_policy)
            .chain(self.inaccessible_at_cut.iter())
            .cloned()
            .collect()
    }
}

/// Panics if the compile refused: every world in this file's fixture set compiles, and a refusal
/// would put the world outside the algebra rather than inside it at a different value.
fn hatches(world: &World, query: &Query) -> Hatches {
    let compiled = bioprism_fiber::compile(world, query).expect("this world compiles");
    let slice = backward_slice(world, query.targets.iter().map(|target| target.as_str()));

    let closure = closure_of(world, query);
    let kept_by_fiber: BTreeSet<String> = slice
        .needed_variables
        .iter()
        .filter_map(|variable| world.fact_providing(variable))
        .map(|fact| fact.id.as_str().to_string())
        .collect();
    let shadowed: BTreeSet<String> = slice
        .needed_variables
        .iter()
        .flat_map(|variable| world.shadowed_provider_ids(variable))
        .filter(|id| !kept_by_fiber.contains(id) && !closure.contains(id))
        .collect();

    Hatches {
        shadowed,
        withheld_by_policy: ids(compiled.trace.policy.withheld.iter().cloned()),
        inaccessible_at_cut: ids(compiled
            .certificate
            .omissions
            .inaccessible_selected_before_cut
            .iter()
            .cloned()),
    }
}

/// Every world in this crate's fixture set that compiles, labelled for assertion messages.
fn compiling_worlds() -> Vec<(String, World, Query)> {
    let mut worlds = vec![(
        "reference-fixture".to_string(),
        reference().0,
        reference().1,
    )];
    for spec in SweepGrid::default_grid().specs() {
        let (world, query) = built(&spec);
        worlds.push((spec.world_id.clone(), world, query));
    }
    for spec in [
        WorldSpec::reference_like(50),
        WorldSpec::discriminating(750),
        WorldSpec::external_confirmation(750),
        WorldSpec::policy_restricted(750),
    ] {
        let (world, query) = built(&spec);
        worlds.push((spec.world_id.clone(), world, query));
    }
    worlds.push({
        let (world, query) = world_with_a_shadowed_provider();
        ("shadowed-provider".to_string(), world, query)
    });
    worlds
}

/// A world in which a needed, unprotected variable has two providers.
///
/// Built from `external_confirmation` with the cut moved past every release, so the shadowing
/// hatch fires with neither pass firing beside it and the difference it produces cannot be
/// mistaken for theirs. `local_lab_value` is the right variable to replicate: the backward slice
/// needs it, no query in this crate protects it, and `WorldSpec::external_confirmation` documents
/// it as the deliberately unprotected control.
///
/// The replicate is appended, so document order makes *it* the survivor and the original the
/// shadowed one — the direction that matters, because a shadowed fact the closure also holds is
/// absorbed and separates nothing.
fn world_with_a_shadowed_provider() -> (World, Query) {
    let spec = WorldSpec::external_confirmation(40).with_decision_time("2025-05-01T00:00:00Z");
    let generated = generate(&spec);
    let mut raw = generated.world;
    let facts = raw["facts"].as_array_mut().expect("facts are an array");
    let mut replicate = facts
        .iter()
        .find(|fact| fact["id"] == LOCAL_LAB_FACT)
        .cloned()
        .expect("the external-confirmation world carries a local lab fact");
    replicate["id"] = json!("fact.local_lab.replicate");
    facts.push(replicate);

    (
        World::from_json(raw).expect("a world with two providers of one variable still loads"),
        Query::from_json(generated.query).expect("generated query loads"),
    )
}

/// Half one of the entailment, over every world this crate can build.
#[test]
fn fibers_selection_is_a_subset_of_the_directed_walks_on_every_world_measured() {
    for (label, world, query) in compiling_worlds() {
        let walk = DirectedDependencyWalk::unbounded()
            .select(&world, &query)
            .facts;
        let compiled = FiberCompiled.select(&world, &query).facts;
        assert!(
            compiled.is_subset(&walk),
            "{label}: fiber selected {:?} the walk did not",
            compiled.difference(&walk).collect::<Vec<_>>()
        );
    }
}

/// Half two, and the sharp form: the difference is not merely explained by the hatches, it *is*
/// them. An equality here rules out a fourth mechanism nobody has named.
#[test]
fn the_selection_difference_is_exactly_the_union_of_the_three_named_escape_hatches() {
    for (label, world, query) in compiling_worlds() {
        let walk = DirectedDependencyWalk::unbounded()
            .select(&world, &query)
            .facts;
        let compiled = FiberCompiled.select(&world, &query).facts;
        let difference: BTreeSet<String> = walk.difference(&compiled).cloned().collect();
        let hatches = hatches(&world, &query);

        assert_eq!(
            difference,
            hatches.union(),
            "{label}: the selections differ by something no named hatch accounts for"
        );
        assert_eq!(
            walk == compiled,
            hatches.none_fired(),
            "{label}: equality must hold exactly when no hatch fires"
        );
    }
}

/// Why the shipped sweep says nothing about these two strategies.
///
/// The grid varies attachment, relay depth, tag style and distractor count. None of the three
/// hatches reads any of the four, so every cell sits on the equality side of the entailment before
/// a single oracle call is made. The 36-of-36 tie `docs/FINDINGS.md` §6 reports as a measurement
/// is this assertion, restated with an unnecessary experiment attached.
#[test]
fn no_cell_of_the_default_grid_fires_any_hatch_so_the_36_of_36_tie_is_entailed() {
    let specs = SweepGrid::default_grid().specs();
    assert_eq!(specs.len(), 36);

    for spec in &specs {
        let (world, query) = built(spec);
        let hatches = hatches(&world, &query);
        assert!(
            hatches.none_fired(),
            "{}: a swept cell fired a hatch, so the grid is no longer provably inert: \
             shadowed {:?}, policy {:?}, cut {:?}",
            spec.world_id,
            hatches.shadowed,
            hatches.withheld_by_policy,
            hatches.inaccessible_at_cut
        );

        let walk = DirectedDependencyWalk::unbounded()
            .select(&world, &query)
            .facts;
        assert_eq!(
            FiberCompiled.select(&world, &query).facts,
            walk,
            "{}",
            spec.world_id
        );
    }
}

/// Escape hatch one: the temporal cut, fired by `events × decision_time`.
///
/// `WorldSpec::external_confirmation` releases `central_lab_confirmation` after the decision time.
/// The walk, which has no cut, keeps the fact; FIBER drops it; and the difference is that one fact
/// and nothing else, which is what makes this a clean separation rather than a general divergence.
#[test]
fn the_temporal_cut_separates_them_and_a_walk_carrying_the_cut_closes_the_gap_exactly() {
    let (world, query) = built(&WorldSpec::external_confirmation(750));
    let walk = DirectedDependencyWalk::unbounded()
        .select(&world, &query)
        .facts;
    let compiled = FiberCompiled.select(&world, &query).facts;

    assert_ne!(walk, compiled, "the cut must fire on this preset");
    assert_eq!(
        walk.difference(&compiled).cloned().collect::<Vec<String>>(),
        vec![CENTRAL_LAB_FACT.to_string()]
    );

    let hatches = hatches(&world, &query);
    assert!(hatches.shadowed.is_empty());
    assert!(
        hatches.withheld_by_policy.is_empty(),
        "the screen is inert here"
    );
    assert_eq!(
        hatches.inaccessible_at_cut,
        ids([CENTRAL_LAB_FACT.to_string()])
    );

    assert_eq!(
        ScreenedDependencyWalk::cut().select(&world, &query).facts,
        compiled,
        "a walk given the cut selects exactly what the compiler does"
    );
    assert_eq!(
        ScreenedDependencyWalk::screened()
            .select(&world, &query)
            .facts,
        walk,
        "and the screen alone changes nothing on a world with no policy requirement"
    );
}

/// Escape hatch two: the policy screen, fired by `policy`.
///
/// `WorldSpec::policy_restricted` inherits the late release *and* adds a clause the query does not
/// accept, so both passes fire on one world and the difference is their union. Only the walk
/// carrying both closes the gap, which is the sharpest statement of what the compiler's selection
/// advantage consists of.
#[test]
fn the_policy_screen_separates_them_and_only_a_walk_carrying_both_passes_closes_the_gap() {
    let (world, query) = built(&WorldSpec::policy_restricted(750));
    let walk = DirectedDependencyWalk::unbounded()
        .select(&world, &query)
        .facts;
    let compiled = FiberCompiled.select(&world, &query).facts;

    assert_eq!(
        walk.difference(&compiled).cloned().collect::<Vec<String>>(),
        vec![CENTRAL_LAB_FACT.to_string(), LOCAL_LAB_FACT.to_string()]
    );

    let hatches = hatches(&world, &query);
    assert_eq!(
        hatches.withheld_by_policy,
        ids([LOCAL_LAB_FACT.to_string()])
    );
    assert_eq!(
        hatches.inaccessible_at_cut,
        ids([CENTRAL_LAB_FACT.to_string()])
    );
    assert!(hatches.shadowed.is_empty());

    assert_ne!(
        ScreenedDependencyWalk::cut().select(&world, &query).facts,
        compiled
    );
    assert_ne!(
        ScreenedDependencyWalk::screened()
            .select(&world, &query)
            .facts,
        compiled
    );
    assert_eq!(
        ScreenedDependencyWalk::compiled()
            .select(&world, &query)
            .facts,
        compiled,
        "both passes together reproduce the compiled selection exactly"
    );
}

/// Escape hatch three: the document-order tiebreak, which no pass can close.
///
/// Neither counter-baseline helps here, and that is the finding. `fact_providing` keeps one
/// provider per variable and the walk keeps every one, so the walk holds a fact FIBER dropped by a
/// tiebreak rather than by a proof — the omission `bioprism-fiber` classes `Unknown` precisely
/// because nobody bounded what the other value would have done to the decision. A benchmark that
/// scored the smaller selection as better would be crediting FIBER for that gap.
#[test]
fn a_shadowed_provider_outside_the_closure_separates_them_with_neither_pass_firing() {
    let (world, query) = world_with_a_shadowed_provider();
    let walk = DirectedDependencyWalk::unbounded()
        .select(&world, &query)
        .facts;
    let compiled = FiberCompiled.select(&world, &query).facts;

    let hatches = hatches(&world, &query);
    assert!(hatches.withheld_by_policy.is_empty());
    assert!(
        hatches.inaccessible_at_cut.is_empty(),
        "the cut was moved past every release so this hatch is isolated"
    );
    assert_eq!(hatches.shadowed, ids([LOCAL_LAB_FACT.to_string()]));

    assert_eq!(
        walk.difference(&compiled).cloned().collect::<Vec<String>>(),
        vec![LOCAL_LAB_FACT.to_string()]
    );
    assert!(compiled.contains("fact.local_lab.replicate"));

    for carrying in [
        ScreenedDependencyWalk::cut(),
        ScreenedDependencyWalk::screened(),
        ScreenedDependencyWalk::compiled(),
    ] {
        assert_eq!(
            carrying.select(&world, &query).facts,
            walk,
            "{} cannot close a gap neither pass opened",
            carrying.name()
        );
    }
}

/// A world whose query accepts a clause the corpus never granted.
///
/// `PolicyEnvelope::resolve` refuses this before any closure or slice runs, which is the one shape
/// in which FIBER produces no selection at all.
fn world_whose_policy_conflicts() -> (World, Query) {
    let spec = WorldSpec {
        policy: PolicySpec {
            governing: vec!["research-only".to_string()],
            accepted: vec!["research-only".to_string(), "never-granted".to_string()],
            requirements: Default::default(),
        },
        ..WorldSpec::discriminating(50)
    };
    built(&spec)
}

/// The refusal, matched on both sides.
///
/// `FiberCompiled` reports a failed compile as an empty selection, so a reader who only saw the
/// fact count would read a refusal as the most compact strategy in the panel. The counter-baseline
/// must fail the same way for the same reason, or the extended panel would rank a walk above a
/// compiler on a world neither of them could serve.
#[test]
fn a_policy_conflict_empties_both_the_compiled_selection_and_the_screened_walks() {
    let (world, query) = world_whose_policy_conflicts();
    assert!(
        bioprism_fiber::compile(&world, &query).is_err(),
        "this world must refuse, or the test asserts nothing"
    );

    let compiled = FiberCompiled.select(&world, &query);
    assert!(compiled.facts.is_empty());
    assert!(
        compiled
            .notes
            .iter()
            .any(|note| note.contains("compile failed")),
        "an empty selection must say it is a refusal: {:?}",
        compiled.notes
    );

    for carrying in [
        ScreenedDependencyWalk::screened(),
        ScreenedDependencyWalk::compiled(),
    ] {
        let selection = carrying.select(&world, &query);
        assert!(
            selection.facts.is_empty(),
            "{} kept a selection the policy envelope refused",
            carrying.name()
        );
        assert!(
            selection.notes.iter().any(|note| note.contains("refused")),
            "{} must report the refusal rather than a fact count: {:?}",
            carrying.name(),
            selection.notes
        );
    }

    assert!(
        !DirectedDependencyWalk::unbounded()
            .select(&world, &query)
            .facts
            .is_empty(),
        "the walk without the screen has no envelope to refuse, which is the gap being measured"
    );
}
