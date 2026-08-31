//! What the section's `goal` says when the query declares none.
//!
//! The CPython reference accepts only `fiber-query/0.1` and `/0.2` and hard-codes a
//! radiogenomic goal into every section without reading the query's `goal` key — a recorded
//! defect reproduced for those versions so the parity bytes cannot move. No later version is
//! accepted by the reference, so no parity claim reaches them, and a missing goal is reported
//! as missing rather than replaced by another domain's sentence.

use bioprism_fiber::{compile, Query, NO_DECLARED_GOAL, REFERENCE_GOAL};
use bioprism_world::World;
use serde_json::{json, Value};
use std::path::PathBuf;

fn fixture(version_dir: &str, relative: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        version_dir,
        relative,
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture is valid JSON")
}

fn golden_world() -> World {
    World::from_json(fixture("fiber-v0.1", "radiogenomic_world.json")).expect("golden world loads")
}

#[test]
fn a_reference_version_query_without_a_goal_keeps_the_reference_substitution() {
    let query = Query::from_json(fixture("fiber-v0.1", "leakage_query.json")).expect("parses");
    assert_eq!(query.goal_text(), REFERENCE_GOAL);
}

#[test]
fn a_post_reference_query_without_a_goal_reports_the_absence_not_another_domains_sentence() {
    let query =
        Query::from_json(fixture("fiber-v0.3", "decision_contract_query.json")).expect("parses");
    assert_eq!(query.goal_text(), NO_DECLARED_GOAL);

    let out = compile(&golden_world(), &query).expect("compiles");
    assert_eq!(out.section.goal, NO_DECLARED_GOAL);
}

#[test]
fn a_declared_goal_is_echoed_verbatim_on_every_version() {
    let mut reference_form = fixture("fiber-v0.1", "leakage_query.json");
    reference_form["goal"] = json!("Decide whether the cohort split is sound.");
    let reference_query = Query::from_json(reference_form).expect("parses");
    assert_eq!(
        reference_query.goal_text(),
        "Decide whether the cohort split is sound."
    );

    let mut decision_form = fixture("fiber-v0.3", "decision_contract_query.json");
    decision_form["goal"] = json!("Decide whether the order flow shows wash trading.");
    let decision_query = Query::from_json(decision_form).expect("parses");
    assert_eq!(
        decision_query.goal_text(),
        "Decide whether the order flow shows wash trading."
    );
}
