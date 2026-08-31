//! The comparison run where the selection algebra says the two strategies can differ.
//!
//! `crates/baseline/tests/selection_equivalence.rs` establishes that `fiber` and
//! `directed-walk-full` can only diverge when the policy screen fires, the temporal cut fires, or
//! a needed variable has a shadowed provider — and that the default sweep grid moves none of the
//! three. `WorldSpec::external_confirmation` and `WorldSpec::policy_restricted` move the first two.
//! This file measures the full panel on both, with the counter-baselines of
//! [`bioprism_baseline::extended_panel`] present, and pins the result.
//!
//! # The result, and it is not a win
//!
//! FIBER selects fewer facts than the naive walk on both worlds — one fewer on
//! `external_confirmation`, two fewer on `policy_restricted` — and every one of those facts is a
//! fact the walk had no mechanism to exclude. Handed the same mechanisms, the walk ties the
//! compiler exactly: `directed-walk-cut` matches FIBER fact-for-fact where only the cut fires, and
//! `directed-walk-compiled` matches it where both do. The measured gap is the two passes, not the
//! compiler, and `docs/FINDINGS.md` §7 states it that way.
//!
//! Nothing here separates the two on *admissibility*: all five walk-family rows and FIBER reach
//! the reference verdict with a complete protected closure on both worlds. The withheld facts are
//! decisive for neither. Cost is the only column that moves, and cost only ranks among admissible
//! strategies — so what these worlds demonstrate is that the passes have an effect, not that the
//! effect changes an answer.

use bioprism_baseline::{compare, extended_panel, Comparison, ContextStrategy, StrategyResult};
use bioprism_fiber::Query;
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};

/// The panel size, asserted so a strategy silently dropped from [`extended_panel`] cannot make a
/// missing row look like an absent finding.
const EXTENDED_PANEL_ROWS: usize = 16;

fn measured(spec: &WorldSpec) -> Comparison {
    let generated = generate(spec);
    let world = World::from_json(generated.world).expect("generated world loads");
    let query = Query::from_json(generated.query).expect("generated query loads");
    let panel = extended_panel();
    let borrowed: Vec<&dyn ContextStrategy> = panel.iter().map(|boxed| boxed.as_ref()).collect();
    compare(&world, &query, &borrowed).expect("both presets reach a full-context verdict")
}

fn row<'a>(comparison: &'a Comparison, name: &str) -> &'a StrategyResult {
    comparison
        .results
        .iter()
        .find(|result| result.name == name)
        .unwrap_or_else(|| panic!("missing row {name}"))
}

/// Facts, soundness, closure and admissibility in one tuple, the four columns the tables carry.
fn columns(comparison: &Comparison, name: &str) -> (usize, Option<bool>, f64, bool) {
    let result = row(comparison, name);
    (
        result.facts_exposed,
        result.verdict_preserving(),
        result.protected_recall,
        result.admissible(),
    )
}

#[test]
fn the_extended_panel_adds_three_rows_to_the_default_one_and_reorders_none_of_it() {
    let default_names: Vec<String> = bioprism_baseline::default_panel()
        .iter()
        .map(|strategy| strategy.name())
        .collect();
    let extended_names: Vec<String> = extended_panel()
        .iter()
        .map(|strategy| strategy.name())
        .collect();

    assert_eq!(extended_names.len(), EXTENDED_PANEL_ROWS);
    assert_eq!(extended_names.len(), default_names.len() + 3);

    let mut remaining = extended_names.iter();
    for expected in &default_names {
        assert!(
            remaining.any(|name| name == expected),
            "the default panel must survive inside the extended one in order; {expected} did not"
        );
    }

    for added in [
        "directed-walk-cut",
        "directed-walk-screened",
        "directed-walk-compiled",
    ] {
        assert!(extended_names.iter().any(|name| name == added));
        assert!(!default_names.iter().any(|name| name == added));
    }
}

/// The temporal-cut world, measured.
///
/// FIBER's twelve facts beat the naive walk's thirteen. `directed-walk-cut` also selects twelve —
/// the same twelve — so the row that separates FIBER from the walk is the row that carries FIBER's
/// pass, and the compiler's own margin over an equally-equipped competitor is zero.
#[test]
fn on_external_confirmation_fiber_beats_the_naive_walk_and_ties_the_walk_carrying_the_cut() {
    let comparison = measured(&WorldSpec::external_confirmation(750));
    assert_eq!(comparison.total_facts, 764);
    assert_eq!(comparison.results.len(), EXTENDED_PANEL_ROWS);

    assert_eq!(columns(&comparison, "fiber"), (12, Some(true), 1.0, true));
    assert_eq!(
        columns(&comparison, "directed-walk-full"),
        (13, Some(true), 1.0, true),
        "the walk with no cut keeps the fact the cut withholds, and is still admissible"
    );
    assert_eq!(
        columns(&comparison, "directed-walk-cut"),
        (12, Some(true), 1.0, true)
    );
    assert_eq!(
        columns(&comparison, "directed-walk-screened"),
        (13, Some(true), 1.0, true),
        "the screen has nothing to withhold on this world"
    );
    assert_eq!(
        columns(&comparison, "directed-walk-compiled"),
        (12, Some(true), 1.0, true)
    );

    assert_eq!(
        comparison.cheapest_admissible().map(|r| r.name.as_str()),
        Some("directed-walk-cut"),
        "the cheapest admissible strategy is a baseline that ties the compiler, not the compiler"
    );

    assert!(
        row(&comparison, "lexical-top-11").protected_recall < 1.0,
        "the retrieval families still fail this world on closure, which is what makes the \
         walk-versus-fiber comparison the only live one"
    );
    assert_eq!(
        row(&comparison, "graph-5-hop").verdict_preserving(),
        Some(false)
    );
}

/// The world where both passes fire.
///
/// The ladder is the whole finding in one column: 13 facts with no pass, 12 with either one, 11
/// with both — and 11 is FIBER. Each pass removes exactly the fact it was written to remove, and a
/// walk carrying both is indistinguishable from the compiler by any column this harness measures.
#[test]
fn on_policy_restricted_each_pass_removes_one_fact_and_the_walk_carrying_both_ties_fiber() {
    let comparison = measured(&WorldSpec::policy_restricted(750));
    assert_eq!(comparison.total_facts, 764);
    assert_eq!(comparison.results.len(), EXTENDED_PANEL_ROWS);

    assert_eq!(columns(&comparison, "fiber"), (11, Some(true), 1.0, true));
    assert_eq!(
        columns(&comparison, "directed-walk-full"),
        (13, Some(true), 1.0, true)
    );
    assert_eq!(
        columns(&comparison, "directed-walk-cut"),
        (12, Some(true), 1.0, true)
    );
    assert_eq!(
        columns(&comparison, "directed-walk-screened"),
        (12, Some(true), 1.0, true)
    );
    assert_eq!(
        columns(&comparison, "directed-walk-compiled"),
        (11, Some(true), 1.0, true)
    );

    assert_eq!(
        comparison.cheapest_admissible().map(|r| r.name.as_str()),
        Some("directed-walk-compiled")
    );
}

/// What the divergence does *not* buy, stated so the cost column is not read as a quality column.
///
/// Every walk-family row and FIBER are admissible on both presets: the withheld facts carry none
/// of the four leakage witnesses, so dropping them costs no soundness and keeping them costs no
/// correctness. A reader who ranked these worlds on verdicts would conclude the passes do nothing.
#[test]
fn the_withheld_facts_are_decisive_for_nobody_so_no_row_changes_its_verdict() {
    for spec in [
        WorldSpec::external_confirmation(750),
        WorldSpec::policy_restricted(750),
    ] {
        let comparison = measured(&spec);
        for name in [
            "fiber",
            "directed-walk-full",
            "directed-walk-cut",
            "directed-walk-screened",
            "directed-walk-compiled",
        ] {
            let result = row(&comparison, name);
            assert_eq!(
                result.verdict_preserving(),
                Some(true),
                "{} on {}",
                name,
                spec.world_id
            );
            assert_eq!(
                result.protected_recall, 1.0,
                "{} on {}",
                name, spec.world_id
            );
        }
        assert!(comparison.is_fully_judged());
    }
}

/// Prints both comparisons as markdown. Ignored because the output, not the assertion, is the
/// point: this is the reproduction path for the tables in `docs/DISCRIMINATING_COMPARISON.md`.
#[test]
#[ignore = "prints the DISCRIMINATING_COMPARISON.md divergence tables; run with -- --ignored --nocapture"]
fn print_the_divergence_region_tables_for_the_comparison_document() {
    for spec in [
        WorldSpec::external_confirmation(750),
        WorldSpec::policy_restricted(750),
    ] {
        println!("{}", measured(&spec).to_markdown());
    }
}
