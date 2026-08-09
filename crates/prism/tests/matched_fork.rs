//! Matched forks, minimization and attested bundles.

use bioprism_fiber::Query;
use bioprism_prism::{
    matched_fork, minimize, minimize_world, preserves, render_table, Acceptance, Architecture,
    Arm, ArmFailure, Attestation, DecisionCell, ForkResult, InputRef, NotAttemptedReason,
    ResultBundle, StrategySpec,
};
use bioprism_section::OracleStatus;
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn reference() -> (Value, Value) {
    let load = |name: &str| -> Value {
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
        serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture")).expect("json")
    };
    (load("radiogenomic_world.json"), load("leakage_query.json"))
}

/// A cell that demands the right verdict, all four witnesses, and full protected closure.
fn leakage_cell(world_json: &Value, query_json: &Value) -> DecisionCell {
    let mut cell = DecisionCell::new(
        "dc_split_integrity_01",
        "does the proposed split support an external-generalization claim?",
        InputRef::new("fixtures/fiber-v0.1/radiogenomic_world.json", world_json),
        InputRef::new("fixtures/fiber-v0.1/leakage_query.json", query_json),
    )
    .accepting(OracleStatus::Invalid);
    for kind in [
        "identity_leakage",
        "site_leakage",
        "temporal_leakage",
        "preprocessing_leakage",
    ] {
        cell = cell.requiring_witness(kind);
    }
    cell
}

#[test]
fn a_cell_binds_its_inputs_by_digest() {
    let (world_json, query_json) = reference();
    let cell = leakage_cell(&world_json, &query_json);

    assert_eq!(
        cell.world.sha256,
        "b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5"
    );
    assert!(cell.world.matches(&world_json));

    let mut altered = world_json.clone();
    altered["world_id"] = Value::String("something-else".into());
    assert!(
        !cell.world.matches(&altered),
        "a cell must not accept a world that is not the one it froze"
    );
}

#[test]
fn acceptance_is_set_valued_and_names_its_failure_mode() {
    let (world_json, query_json) = reference();
    let cell = leakage_cell(&world_json, &query_json);
    let all: BTreeSet<String> = [
        "identity_leakage",
        "site_leakage",
        "temporal_leakage",
        "preprocessing_leakage",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    assert!(cell.accepts(OracleStatus::Invalid, &all, true).passed());

    assert!(matches!(
        cell.accepts(OracleStatus::Valid, &all, true),
        Acceptance::WrongVerdict { .. }
    ));

    let partial: BTreeSet<String> = ["identity_leakage".to_string()].into_iter().collect();
    assert!(matches!(
        cell.accepts(OracleStatus::Invalid, &partial, true),
        Acceptance::MissingWitnesses(_)
    ));

    assert!(matches!(
        cell.accepts(OracleStatus::Invalid, &all, false),
        Acceptance::ClosureIncomplete
    ));
}

#[test]
fn on_the_reference_world_the_panel_does_not_separate() {
    let (world_json, query_json) = reference();
    let cell = leakage_cell(&world_json, &query_json);
    let world = World::from_json(world_json).unwrap();
    let query = Query::from_json(query_json).unwrap();

    let result = matched_fork(&cell, &world, &query, &Architecture::default_panel());

    assert!(
        result.is_regression_free(),
        "every architecture should pass here: {:?}",
        result.failing
    );
    assert!(result.attribution.contains("every architecture"));
    assert_eq!(result.cheapest_passing().unwrap().facts_exposed, 11);
}

/// The discriminating world turns the same panel into a real experiment.
#[test]
fn on_the_discriminating_world_context_policy_explains_the_difference() {
    let spec = WorldSpec::discriminating(750);
    let generated = generate(&spec);
    let world_json = generated.world.clone();
    let query_json = generated.query.clone();

    let mut cell = DecisionCell::new(
        "dc_discriminating_01",
        "split integrity under camouflaged distractors",
        InputRef::new("generated/world.json", &world_json),
        InputRef::new("generated/query.json", &query_json),
    )
    .accepting(OracleStatus::Invalid);
    for kind in [
        "identity_leakage",
        "site_leakage",
        "temporal_leakage",
        "preprocessing_leakage",
    ] {
        cell = cell.requiring_witness(kind);
    }

    let world = World::from_json(world_json).unwrap();
    let query = Query::from_json(query_json).unwrap();
    let result = matched_fork(&cell, &world, &query, &Architecture::default_panel());

    assert!(result.passing.contains(&"fiber".to_string()));
    assert!(result.failing.contains(&"graph-5-hop".to_string()));
    assert!(result.failing.contains(&"lexical-top-11".to_string()));
    assert!(result.attribution.contains("context policy explains"));
    assert!(
        result.attribution.contains("incomplete protected closure"),
        "the lucky-but-incomplete failure must be named: {}",
        result.attribution
    );

    let cheapest = result.cheapest_passing().unwrap();
    assert_eq!(cheapest.architecture, "fiber");
    assert_eq!(cheapest.facts_exposed, 11);

    let table = render_table(&result);
    assert!(table.contains("| fiber |"));
    assert!(table.contains("**fail**"));
}

/// The reference world with one aliased subject's split arm removed.
///
/// `S001` and `S003` share alias `ALT-77`; with `S003` unassigned the split-integrity oracle has a
/// group it cannot order, so any arm selecting both the alias table and the split table gets no
/// verdict at all. An arm selecting neither is unaffected, which is what makes the two states
/// observable in one panel.
fn unjudgeable() -> (Value, Value) {
    let (mut world_json, query_json) = reference();
    world_json["facts"]
        .as_array_mut()
        .expect("facts")
        .iter_mut()
        .find(|fact| fact["id"] == Value::String("fact.split".into()))
        .expect("the split fact")["value"]
        .as_object_mut()
        .expect("split assignments")
        .remove("S003")
        .expect("S003 shares ALT-77 with S001");
    (world_json, query_json)
}

fn fork_over(world_json: Value, query_json: Value, panel: &[Architecture]) -> ForkResult {
    let cell = leakage_cell(&world_json, &query_json);
    let world = World::from_json(world_json).unwrap();
    let query = Query::from_json(query_json).unwrap();
    matched_fork(&cell, &world, &query, panel)
}

#[test]
fn an_arm_that_failed_is_not_reported_as_an_arm_that_never_ran() {
    let (world_json, query_json) = unjudgeable();
    let panel = [
        Architecture::new("full-context", StrategySpec::FullContext),
        Architecture::new("lexical-top-1", StrategySpec::LexicalTopK { k: 1 }),
        Architecture::new("full-context", StrategySpec::LexicalTopK { k: 2 }),
    ];
    let result = fork_over(world_json, query_json, &panel);

    match &result.arms[..] {
        [Arm::Unjudged { architecture, reason }, Arm::Judged(trial), Arm::NotAttempted { reason: skipped, .. }] =>
        {
            assert_eq!(architecture, "full-context");
            let ArmFailure::OracleRefused { facts_exposed, detail } = reason;
            assert_eq!(*facts_exposed, 761, "the cost of the attempt is still reported");
            assert!(
                detail.contains("ALT-77"),
                "the oracle's own refusal must survive: {detail}"
            );
            assert!(!trial.passed, "one fact cannot satisfy a four-witness cell");
            assert_eq!(*skipped, NotAttemptedReason::DuplicateArchitectureName);
        }
        other => panic!("expected one arm of each state, got {other:?}"),
    }

    assert_eq!(
        result.failing,
        vec!["lexical-top-1".to_string()],
        "an arm the oracle refused is not an arm that failed the cell"
    );
    assert_eq!(result.unjudged, vec!["full-context".to_string()]);
    assert_eq!(result.not_attempted, vec!["full-context".to_string()]);
    assert!(result.passing.is_empty());
    assert!(!result.is_regression_free());

    assert!(
        result.attribution.contains("produced no verdict at all"),
        "an arm with no verdict must appear in the sentence: {}",
        result.attribution
    );
    assert!(
        result.attribution.contains("never attempted"),
        "an arm nobody ran must appear in the sentence: {}",
        result.attribution
    );

    let table = render_table(&result);
    assert!(table.contains("**unjudged**"));
    assert!(table.contains("*not attempted*"));
}

/// The panel is what the fork was asked to check, so an arm missing from it cannot pass silently.
#[test]
fn an_arm_that_never_ran_is_not_counted_towards_a_clean_panel() {
    let (world_json, query_json) = reference();
    let mut panel = Architecture::default_panel();
    panel.push(Architecture::new("fiber", StrategySpec::FullContext));

    let result = fork_over(world_json, query_json, &panel);

    assert_eq!(result.arms.len(), 6, "every architecture asked for gets an arm");
    assert_eq!(result.passing.len(), 5);
    assert_eq!(
        result.passing.iter().filter(|name| *name == "fiber").count(),
        1,
        "a name cannot appear twice on the passing side"
    );
    assert_eq!(result.not_attempted, vec!["fiber".to_string()]);
    assert!(result.failing.is_empty());
    assert!(
        !result.is_regression_free(),
        "an arm nobody ran leaves the panel unchecked, whatever the other five did"
    );
}

#[test]
fn a_fork_with_no_arms_does_not_report_itself_regression_free() {
    let (world_json, query_json) = reference();
    let result = fork_over(world_json, query_json, &[]);

    assert!(result.arms.is_empty());
    assert!(
        !result.is_regression_free(),
        "checking nothing is not the same as finding nothing"
    );
    assert!(result.attribution.contains("establishes nothing"));
}

#[test]
fn every_arm_state_survives_serialization_and_only_a_judged_arm_carries_numbers() {
    let (world_json, query_json) = unjudgeable();
    let panel = [
        Architecture::new("full-context", StrategySpec::FullContext),
        Architecture::new("lexical-top-1", StrategySpec::LexicalTopK { k: 1 }),
        Architecture::new("full-context", StrategySpec::LexicalTopK { k: 2 }),
    ];
    let result = fork_over(world_json, query_json, &panel);

    let encoded = serde_json::to_value(&result).unwrap();
    let arms = encoded["arms"].as_array().unwrap();
    assert_eq!(arms[0]["state"], Value::String("unjudged".into()));
    assert_eq!(arms[1]["state"], Value::String("judged".into()));
    assert_eq!(arms[2]["state"], Value::String("not_attempted".into()));

    for absent in ["passed", "facts_exposed", "status", "protected_recall"] {
        assert!(
            arms[0].get(absent).is_none() && arms[2].get(absent).is_none(),
            "an arm with no verdict must not carry {absent} for a renderer to coerce"
        );
        assert!(arms[1].get(absent).is_some(), "a judged arm carries {absent}");
    }

    let decoded: ForkResult = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, result, "the three states must round-trip");
}

#[test]
fn minimization_reduces_the_world_and_preserves_the_signature() {
    let (world_json, _) = reference();
    let world = World::from_json(world_json).unwrap();

    let result = minimize_world(&world);

    assert_eq!(result.started_from, 761);
    assert!(
        result.minimal.len() < 20,
        "expected a small diagnosis, got {}",
        result.minimal.len()
    );
    assert_eq!(result.preserved_status, "invalid");
    assert_eq!(result.preserved_witnesses.len(), 4);
    assert!(result.reduction_ratio() < 0.03);
    assert!(preserves(&world, &result), "minimal set must re-verify");
    assert!(result.guarantee.contains("1-minimal"));
    assert!(result.guarantee.contains("Not globally minimal"));
}

#[test]
fn every_fact_in_the_minimal_set_is_load_bearing() {
    let (world_json, _) = reference();
    let world = World::from_json(world_json).unwrap();
    let result = minimize_world(&world);

    let minimal: BTreeSet<String> = result.minimal.iter().cloned().collect();
    for id in &minimal {
        let mut without = minimal.clone();
        without.remove(id);
        let again = minimize(&world, &without);
        assert_ne!(
            (again.preserved_status.as_str(), again.preserved_witnesses.len()),
            ("invalid", 4),
            "removing {id} should have changed the signature, so it was not load-bearing"
        );
    }
}

#[test]
fn minimization_is_deterministic() {
    let (world_json, _) = reference();
    let world = World::from_json(world_json).unwrap();
    assert_eq!(minimize_world(&world).minimal, minimize_world(&world).minimal);
}

#[test]
fn a_bundle_attests_itself_and_detects_tampering() {
    let (world_json, query_json) = reference();
    let cell = leakage_cell(&world_json, &query_json);
    let world = World::from_json(world_json).unwrap();
    let query = Query::from_json(query_json).unwrap();

    let fork = matched_fork(&cell, &world, &query, &Architecture::default_panel());
    let bundle = ResultBundle::new(cell, fork).with_minimization(minimize_world(&world));

    let attested = bundle.attest();
    assert!(ResultBundle::verify(&attested).is_valid());
    assert!(attested["reproduction"]["command"]
        .as_str()
        .unwrap()
        .starts_with("bioprism context compare"));
    assert!(
        attested["limitations"].as_array().unwrap().len() >= 2,
        "a bundle must state what it does not establish"
    );

    let mut tampered = attested.clone();
    tampered["fork"]["attribution"] = Value::String("fiber wins everywhere".into());
    match ResultBundle::verify(&tampered) {
        Attestation::Mismatch { .. } => {}
        other => panic!("tampering must be detected, got {other:?}"),
    }

    let mut stripped = attested;
    stripped.as_object_mut().unwrap().remove("bundle_sha256");
    assert!(matches!(
        ResultBundle::verify(&stripped),
        Attestation::Malformed(_)
    ));
}

#[test]
fn architectures_round_trip_through_their_specs() {
    for spec in [
        StrategySpec::Fiber,
        StrategySpec::FullContext,
        StrategySpec::GraphKHop { depth: 5 },
        StrategySpec::ConnectedComponent,
        StrategySpec::QueryGraph,
        StrategySpec::LexicalTopK { k: 11 },
    ] {
        let encoded = serde_json::to_value(&spec).unwrap();
        let decoded: StrategySpec = serde_json::from_value(encoded).unwrap();
        assert_eq!(spec, decoded);
        assert!(!decoded.build().name().is_empty());
    }
}
