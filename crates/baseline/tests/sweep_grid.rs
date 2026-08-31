//! The structural family sweep — the experiment FINDINGS.md §4 said was missing.
//!
//! The full default grid (36 cells) runs in about seven seconds in a debug build, well under the
//! suite's tolerance, so the headline aggregate is asserted against the *full* grid here rather
//! than a sample; only the determinism check uses a reduced grid, because it runs the sweep
//! twice. The ignored test at the bottom prints the complete markdown table and is how the
//! numbers in `docs/FINDINGS.md` §6 were produced:
//! `cargo test -p bioprism-baseline --offline --test sweep_grid -- --ignored --nocapture`.

use bioprism_baseline::sweep::{run_cell, run_sweep, SweepGrid};
use bioprism_worldgen::{DistractorAttachment, TagStyle, WorldSpec};

fn reduced_grid() -> SweepGrid {
    SweepGrid {
        attachments: vec![DistractorAttachment::Hub, DistractorAttachment::NearTarget],
        relay_depths: vec![0, 2],
        tag_styles: vec![TagStyle::Distinct, TagStyle::Camouflaged],
        distractor_counts: vec![50],
        seed: 20_260_823,
    }
}

#[test]
fn the_same_grid_and_seed_produce_an_identical_table_and_identical_markdown_twice() {
    let grid = reduced_grid();
    let first = run_sweep(&grid).expect("every cell reaches a reference verdict");
    let second = run_sweep(&grid).expect("every cell reaches a reference verdict");
    assert_eq!(first, second);
    assert_eq!(first.to_markdown(), second.to_markdown());
}

/// The discriminating preset, run through the sweep's own cell machinery, must reproduce the
/// rows `docs/FINDINGS.md` §3 documents for the pre-existing strategies. If this fails, either
/// the sweep measures something different from `bioprism context compare` or the documented
/// numbers have drifted; both are reportable.
#[test]
fn the_discriminating_preset_cell_reproduces_the_documented_findings_rows() {
    let cell = run_cell(&WorldSpec::discriminating(750)).expect("the preset reaches a verdict");
    assert_eq!(cell.total_facts, 762);

    let row = |name: &str| cell.row(name).unwrap_or_else(|| panic!("missing row {name}"));

    let full = row("full-context");
    assert_eq!((full.facts_selected, full.sound, full.admissible), (762, Some(true), true));

    let graph = row("graph-5-hop");
    assert_eq!((graph.facts_selected, graph.sound), (750, Some(false)));
    assert_eq!(graph.protected_closure, 0.0);

    let lexical = row("lexical-top-11");
    assert_eq!((lexical.facts_selected, lexical.sound), (11, Some(true)));
    assert!((lexical.protected_closure - 10.0 / 11.0).abs() < 1e-12);
    assert!(!lexical.admissible, "right verdict from an incomplete closure is not a pass");

    let fiber = row("fiber");
    assert_eq!(
        (fiber.facts_selected, fiber.sound, fiber.protected_closure, fiber.admissible),
        (11, Some(true), 1.0, true)
    );

    let walk = row("directed-walk-full");
    assert_eq!(
        (walk.facts_selected, walk.sound, walk.protected_closure, walk.admissible),
        (11, Some(true), 1.0, true),
        "the directed walk ties fiber on the world built to discriminate"
    );
}

/// The full default grid, measured, wherever it lands — and where it lands is against FIBER's
/// uniqueness: the directed dependency walk is admissible in every one of the 36 cells at exactly
/// FIBER's fact count. No structural corner in the swept family separates them. Every other
/// family has cells it fails: the undirected walk dies with any relay chain, and both retrieval
/// baselines die under camouflage.
#[test]
fn across_all_36_cells_the_directed_walk_matches_fiber_and_no_other_compact_strategy_survives() {
    let table = run_sweep(&SweepGrid::default_grid()).expect("every cell reaches a verdict");
    assert_eq!(table.cells.len(), 36);

    for cell in &table.cells {
        let fiber = cell.row("fiber").expect("fiber row");
        assert!(fiber.admissible, "fiber must be admissible in {}", cell.world_id);
        assert_eq!(fiber.facts_selected, 11, "fiber compiles 11 facts in {}", cell.world_id);

        let walk = cell.row("directed-walk-full").expect("walk row");
        assert!(walk.admissible, "the walk ties fiber in {}", cell.world_id);
        assert_eq!(walk.facts_selected, fiber.facts_selected, "in {}", cell.world_id);
    }

    assert_eq!(table.admissible_cells("full-context"), 36);
    assert_eq!(table.admissible_cells("graph-4-hop"), 0);
    // The undirected walk survives exactly the twelve relay-free cells; one relay step past its
    // depth window removes all of them.
    assert_eq!(table.admissible_cells("graph-5-hop"), 12);
    assert_eq!(table.admissible_cells("graph-6-hop"), 12);
    assert_eq!(table.admissible_cells("graph-7-hop"), 12);
    for cell in &table.cells {
        let graph = cell.row("graph-5-hop").expect("graph row");
        assert_eq!(
            graph.admissible,
            cell.relay_depth == 0,
            "graph-5-hop admissibility is exactly the relay knob in {}",
            cell.world_id
        );
    }

    // Retrieval: every distinct-tag cell is winnable by at least one budget, no camouflaged cell
    // at 250+ distractors is winnable by any measured one. lexical-top-11 additionally loses the
    // small distinct corpora, where 50 documents' IDF no longer isolates the protected tags.
    assert_eq!(table.admissible_cells("lexical-top-11"), 12);
    assert_eq!(table.admissible_cells("lexical-top-50"), 18);
    assert_eq!(table.admissible_cells("embedding-top-11"), 18);
    assert_eq!(table.admissible_cells("embedding-top-50"), 24);
    for cell in &table.cells {
        for name in ["lexical-top-11", "lexical-top-50", "embedding-top-11", "embedding-top-50"] {
            let row = cell.row(name).expect("retrieval row");
            if cell.tag_style == TagStyle::Camouflaged && cell.distractors >= 250 {
                assert!(!row.admissible, "{name} must fail {}", cell.world_id);
            }
            if cell.tag_style == TagStyle::Distinct && name != "lexical-top-11" {
                assert!(row.admissible, "{name} must survive {}", cell.world_id);
            }
        }
    }
}

/// Prints the full grid as markdown. Ignored because its output, not its assertion, is the
/// point: this is the reproduction path for the table recorded in `docs/FINDINGS.md` §6.
#[test]
#[ignore = "prints the FINDINGS.md sweep table; run with -- --ignored --nocapture"]
fn print_the_full_default_grid_markdown_for_the_findings_document() {
    let table = run_sweep(&SweepGrid::default_grid()).expect("every cell reaches a verdict");
    println!("{}", table.to_markdown());
}
