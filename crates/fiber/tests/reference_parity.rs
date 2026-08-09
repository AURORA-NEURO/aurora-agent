//! Byte-level parity with the CPython reference runtime.
//!
//! The acceptance gate for Wave 1 (43.42) is that a clean checkout compiles the toy world and
//! produces a deterministic certificate. Here that is strengthened: the Rust compiler must emit
//! the *same bytes* as `reference/fiber_runtime/fiber_compile.py`, so a certificate produced by
//! either implementation replays against the other.
//!
//! Regenerate the golden artifacts with `python tools/regenerate_golden.py`.

use bioprism_fiber::{backward_slice, compile, temporal_cut, FiberError, Query};
use bioprism_ids::to_canonical_string;
use bioprism_scope::Timestamp;
use bioprism_section::{CertificateProfile, InfluenceClass, OracleStatus};
use bioprism_world::World;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn fixture(relative: &str) -> Value {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "fiber-v0.1", relative]
        .iter()
        .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture is valid JSON")
}

fn golden_world() -> World {
    World::from_json(fixture("radiogenomic_world.json")).expect("golden world loads")
}

fn leakage_query() -> Query {
    Query::from_json(fixture("leakage_query.json")).expect("golden query loads")
}

#[test]
fn decision_section_is_byte_identical_to_the_reference() {
    let out = compile(&golden_world(), &leakage_query()).expect("compiles");
    let expected = to_canonical_string(&fixture("golden/reference_section.json")).unwrap();
    let actual = out.section.to_canonical_string().unwrap();
    assert_eq!(actual, expected, "decision section diverged from CPython");
}

#[test]
fn certificate_is_byte_identical_to_the_reference() {
    let out = compile(&golden_world(), &leakage_query()).expect("compiles");
    let expected = to_canonical_string(&fixture("golden/reference_certificate.json")).unwrap();
    let actual = to_canonical_string(&out.certificate.to_json(CertificateProfile::Reference).unwrap())
        .unwrap();
    assert_eq!(actual, expected, "certificate diverged from CPython");
}

#[test]
fn certificate_digest_matches_the_published_value() {
    let out = compile(&golden_world(), &leakage_query()).expect("compiles");
    assert_eq!(
        out.certificate.digest(CertificateProfile::Reference).unwrap().as_str(),
        "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4"
    );
    assert_eq!(
        out.certificate.source_hashes.decision_section_sha256,
        "7439b2262c52c1c794b59be86d922b723a2ea5646362d529f57fb11b5f7e93ce"
    );
    assert_eq!(
        out.certificate.source_hashes.world_sha256,
        "b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5"
    );
}

#[test]
fn compilation_is_deterministic() {
    let world = golden_world();
    let query = leakage_query();
    let first = compile(&world, &query).unwrap();
    let second = compile(&world, &query).unwrap();
    assert_eq!(
        first.certificate.to_json(CertificateProfile::Reference).unwrap(),
        second.certificate.to_json(CertificateProfile::Reference).unwrap()
    );
}

#[test]
fn protected_closure_is_retained() {
    let out = compile(&golden_world(), &leakage_query()).unwrap();
    let selected: BTreeSet<&str> = out
        .certificate
        .selected_facts
        .iter()
        .map(String::as_str)
        .collect();
    for required in [
        "fact.subject_aliases",
        "fact.split",
        "fact.site",
        "fact.label_source",
        "fact.decision_cut",
        "fact.preprocess_fit",
        "fact.specimen_dates",
        "fact.policy",
        "fact.negative_duplicates",
    ] {
        assert!(selected.contains(required), "protected fact {required} was dropped");
    }
    assert!(out.protected_closure_satisfied());
    assert!(out.trace.unmatched_protected_tags.is_empty());
}

#[test]
fn exploratory_hub_is_not_followed_forward() {
    let out = compile(&golden_world(), &leakage_query()).unwrap();
    assert!(
        !out.certificate
            .selected_facts
            .iter()
            .any(|id| id.starts_with("fact.explore.")),
        "no exploratory fact may be selected"
    );
    assert_eq!(out.certificate.omissions.exploratory_facts, 750);
    assert_eq!(out.certificate.omissions.total_facts, 750);
}

/// Expanding around the compiled selection re-admits the whole world.
///
/// Every one of the 750 exploratory factors consumes `cohort_id`, a protected fact that *is*
/// selected. So a single undirected hop out from the selected variables covers 761 of 761 facts.
///
/// This says something narrower than "graphs lose": it says the compiled selection sits directly
/// against a hub, so any *widening* step — a verification pass, a "grab the neighbours too"
/// heuristic, a human clicking expand — goes straight from 11 facts to the entire world with no
/// intermediate stop. It is a statement about the danger of expansion, not about whether graph
/// retrieval can find the answer. It can: see `crates/baseline`, where a 5-hop walk from the
/// target selects exactly this set.
#[test]
fn expanding_around_the_selection_re_admits_the_whole_world() {
    let world = golden_world();
    let out = compile(&world, &leakage_query()).unwrap();

    let selected_variables: BTreeSet<&str> = out
        .certificate
        .selected_facts
        .iter()
        .filter_map(|id| world.fact(id))
        .map(|fact| fact.provides.as_str())
        .collect();

    let mut one_hop: BTreeSet<&str> = BTreeSet::new();
    for factor in &world.factors {
        let touches = factor
            .inputs
            .iter()
            .chain(factor.outputs.iter())
            .any(|v| selected_variables.contains(v.as_str()));
        if touches {
            for v in factor.inputs.iter().chain(factor.outputs.iter()) {
                one_hop.insert(v.as_str());
            }
        }
    }
    let neighbourhood_facts = world
        .facts
        .iter()
        .filter(|fact| one_hop.contains(fact.provides.as_str()))
        .count();

    assert_eq!(neighbourhood_facts, 761, "the hub really does reach everything");
    assert_eq!(out.certificate.selected_facts.len(), 11);
    assert!(
        out.certificate.plan.fact_selection_ratio() < 0.05,
        "reduction claim of 43.41 not met: {}",
        out.certificate.plan.fact_selection_ratio()
    );
}

#[test]
fn all_four_leakage_mechanisms_are_detected_with_witnesses() {
    let out = compile(&golden_world(), &leakage_query()).unwrap();
    let verdict = &out.certificate.oracle;
    assert_eq!(verdict.status, OracleStatus::Invalid);
    assert_eq!(
        verdict.witness_kinds(),
        vec![
            "identity_leakage",
            "site_leakage",
            "temporal_leakage",
            "preprocessing_leakage"
        ]
    );
}

#[test]
fn the_temporal_cut_withholds_the_future_label() {
    let world = golden_world();
    let at_training = temporal_cut(&world, Timestamp::parse("2025-01-01T00:00:00Z").unwrap());
    assert!(!at_training.is_accessible("future_label_value"));
    assert!(at_training.is_accessible("split_assignment"));
    assert_eq!(at_training.withheld().count(), 1);

    let retrospective = temporal_cut(&world, Timestamp::parse("2026-01-01T00:00:00Z").unwrap());
    assert!(retrospective.is_accessible("future_label_value"));
    assert_eq!(retrospective.withheld().count(), 0);
}

#[test]
fn slicing_terminates_and_selects_only_reachable_factors() {
    let world = golden_world();
    let slice = backward_slice(&world, ["split_integrity_status"]);
    assert_eq!(slice.selected_factors.len(), 6);
    assert!(slice.selected_factors.contains("factor.claim_support"));
    assert!(slice
        .selected_factors
        .iter()
        .all(|id| !id.starts_with("factor.explore.")));
}

#[test]
fn a_budget_smaller_than_the_protected_closure_is_a_hard_failure() {
    let mut raw = fixture("leakage_query.json");
    raw["budgets"]["max_facts"] = serde_json::json!(4);
    let query = Query::from_json(raw).unwrap();
    match compile(&golden_world(), &query) {
        Err(FiberError::BudgetExceeded { selected, max_facts }) => {
            assert_eq!(selected, 11);
            assert_eq!(max_facts, 4);
        }
        other => panic!("must refuse rather than truncate protected evidence, got {other:?}"),
    }
}

#[test]
fn the_extended_profile_classifies_omissions_by_influence() {
    let out = compile(&golden_world(), &leakage_query()).unwrap();
    let manifest = &out.certificate.manifest;
    assert_eq!(manifest.total_omitted(), 750);
    assert_eq!(manifest.count_in(InfluenceClass::Zero), 750);
    assert_eq!(manifest.count_in(InfluenceClass::Unknown), 0);
    assert!(manifest.supports_sufficiency_claim());

    let extended = out.certificate.to_json(CertificateProfile::Extended).unwrap();
    assert_eq!(extended["supports_sufficiency_claim"], serde_json::json!(true));
}

#[test]
fn the_compiler_reports_the_passes_it_could_not_run() {
    let out = compile(&golden_world(), &leakage_query()).unwrap();
    let deferred: Vec<&str> = out.trace.deferred_passes.iter().map(|(name, _)| *name).collect();
    for expected in [
        "obstruction_tests",
        "abstract_interpretation",
        "role_and_purpose_filter",
        "information_flow_export",
        "decision_quotient",
        "rate_distortion",
    ] {
        assert!(
            deferred.contains(&expected),
            "pass {expected} must be declared deferred, not silently skipped"
        );
    }

    let executed: Vec<&str> = out.trace.passes.iter().map(|p| p.name).collect();
    assert_eq!(
        executed,
        vec![
            "protected_closure",
            "backward_slice",
            "policy",
            "temporal_cut",
            "oracle",
            "plan_selection",
            "influence_bounds"
        ]
    );
}

#[test]
fn a_wrong_query_schema_is_rejected() {
    let mut raw = fixture("leakage_query.json");
    raw["schema_version"] = serde_json::json!("fiber-query/0.9");
    assert!(matches!(
        Query::from_json(raw),
        Err(FiberError::UnsupportedQuerySchema { .. })
    ));
}
