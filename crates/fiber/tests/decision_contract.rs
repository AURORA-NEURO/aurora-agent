//! Executable `fiber-query/0.3` and `fiber-query/0.4` coverage for the decision and context
//! boundaries in blueprint 43.02, 43.10 and 43.12.
//!
//! These tests intentionally exercise the published fixture rather than constructing a private
//! Rust-only query. A wire contract is real only when its JSON, parser, compiler trace, certificate
//! identity, and downstream quotient all agree on the same bytes and refusal semantics.

use bioprism_fiber::{compile, FiberError, Query, QUERY_FIELD_PATHS};
use bioprism_section::CertificateProfile;
use bioprism_world::World;
use serde_json::{json, Value};
use std::path::PathBuf;

fn fixture(relative: &str) -> Value {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", relative]
        .iter()
        .collect();
    serde_json::from_str(
        &std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("missing fixture {}: {error}", path.display())),
    )
    .expect("fixture is valid JSON")
}

fn decision_query() -> Value {
    fixture("fixtures/fiber-v0.3/decision_contract_query.json")
}

fn rate_distortion_query() -> Value {
    fixture("fixtures/fiber-v0.4/rate_distortion_query.json")
}

fn reference_world() -> World {
    World::from_json(fixture("fixtures/fiber-v0.1/radiogenomic_world.json"))
        .expect("reference world loads")
}

type QueryMutation = fn(&mut Value);

#[test]
fn version_03_executes_the_quotient_and_keeps_rate_distortion_deferred() {
    let query = Query::from_json(decision_query()).expect("0.3 query parses");
    let contract = query
        .decision_contract
        .as_ref()
        .expect("0.3 carries a decision contract");
    assert_eq!(contract.problem.action_count(), 3);
    assert_eq!(contract.problem.model_count(), 3);
    assert_eq!(contract.sense.as_str(), "loss");

    let out = compile(&reference_world(), &query).expect("0.3 query compiles");
    let quotient = out
        .trace
        .decision_quotient
        .as_ref()
        .expect("the compiler ran the 43.10 quotient");
    assert_eq!(
        quotient.permitted_actions,
        vec![
            "accept".to_string(),
            "defer".to_string(),
            "reject".to_string()
        ]
    );
    assert_eq!(quotient.original_model_count, 3);
    assert_eq!(quotient.quotient_model_count, 2);
    assert_eq!(quotient.merged_model_count, 1);
    assert_eq!(
        quotient.class_for_model("m-a"),
        quotient.class_for_model("m-b")
    );
    assert_ne!(
        quotient.class_for_model("m-a"),
        quotient.class_for_model("m-c")
    );

    assert!(out
        .trace
        .passes
        .iter()
        .any(|pass| pass.name == "decision_quotient" && pass.retained == 2));
    assert!(!out
        .trace
        .deferred_passes
        .iter()
        .any(|(name, _)| *name == "decision_quotient"));
    assert!(out
        .trace
        .deferred_passes
        .iter()
        .any(|(name, reason)| *name == "rate_distortion" && reason.contains("evidence-pool")));
}

#[test]
fn utility_sense_is_converted_once_at_the_wire_boundary() {
    let mut raw = decision_query();
    raw["decision_loss"]["sense"] = json!("utility");
    let query = Query::from_json(raw).expect("utility query parses");
    let contract = query.decision_contract.expect("contract is present");
    assert_eq!(contract.sense.as_str(), "utility");
    assert_eq!(contract.problem.loss(0, 1), -7.0);
    assert_eq!(contract.problem.loss(1, 0), -4.0);
}

#[test]
fn version_04_executes_identification_frontier_and_minimal_context() {
    let query = Query::from_json(rate_distortion_query()).expect("0.4 query parses");
    let contract = query
        .rate_distortion
        .as_ref()
        .expect("0.4 carries a rate-distortion contract");
    assert_eq!(contract.evidence_pool.len(), 2);
    assert_eq!(contract.prior.len(), 3);
    assert_eq!(contract.compatibility_floor, 0.05);

    let out = compile(&reference_world(), &query).expect("0.4 query compiles");
    let report = out
        .trace
        .rate_distortion
        .as_ref()
        .expect("the compiler ran the 43.12 context audit");
    assert_eq!(report.frontier.evaluated, 4);
    assert_eq!(report.evidence_count, 2);
    assert!(report.full_rate > 0.0);
    assert!(out
        .trace
        .passes
        .iter()
        .any(|pass| pass.name == "rate_distortion" && pass.retained <= 2));
    assert!(!out
        .trace
        .deferred_passes
        .iter()
        .any(|(name, _)| *name == "rate_distortion"));
}

#[test]
fn changing_observed_evidence_changes_rate_distortion_identity() {
    let world = reference_world();
    let first = Query::from_json(rate_distortion_query()).expect("first query parses");
    let first_out = compile(&world, &first).expect("first query compiles");

    let mut changed_raw = rate_distortion_query();
    changed_raw["rate_distortion"]["evidence_pool"]["items"][0]["cost"] = json!(3.0);
    let changed = Query::from_json(changed_raw).expect("changed query parses");
    let changed_out = compile(&world, &changed).expect("changed query compiles");

    assert_ne!(
        first_out.certificate.source_hashes.query_sha256,
        changed_out.certificate.source_hashes.query_sha256,
        "observed evidence bindings are certificate inputs"
    );
}

#[test]
fn changing_the_decision_contract_changes_certificate_identity() {
    let world = reference_world();
    let first = Query::from_json(decision_query()).expect("first query parses");
    let first_out = compile(&world, &first).expect("first query compiles");

    let mut changed_raw = decision_query();
    changed_raw["decision_loss"]["loss"][1][2] = json!(6.0);
    let changed = Query::from_json(changed_raw).expect("changed query parses");
    let changed_out = compile(&world, &changed).expect("changed query compiles");

    assert_ne!(
        first_out.certificate.source_hashes.query_sha256,
        changed_out.certificate.source_hashes.query_sha256,
        "the query bytes, including the loss contract, are certificate inputs"
    );
    assert_ne!(
        first_out
            .certificate
            .digest(CertificateProfile::Reference)
            .expect("first digest"),
        changed_out
            .certificate
            .digest(CertificateProfile::Reference)
            .expect("changed digest")
    );
}

#[test]
fn legacy_versions_refuse_decision_fields_instead_of_gaining_implicit_semantics() {
    let mut raw = decision_query();
    raw["schema_version"] = json!("fiber-query/0.2");
    let error = Query::from_json(raw).expect_err("0.2 must not accept 0.3 fields");
    match error {
        FiberError::UnknownQueryFields { fields, accepted } => {
            assert!(fields.contains(&"decision_loss".to_string()));
            assert!(fields.contains(&"permitted_actions".to_string()));
            assert_eq!(accepted, QUERY_FIELD_PATHS);
        }
        other => panic!("expected the versioned refusal, got {other:?}"),
    }
}

#[test]
fn malformed_contracts_are_refused_before_compilation() {
    let cases: [(&str, QueryMutation); 6] = [
        ("missing permitted action", |raw: &mut Value| {
            raw.as_object_mut().unwrap().remove("permitted_actions");
        }),
        ("wrong row count", |raw: &mut Value| {
            raw["decision_loss"]["loss"] = json!([[0.0, 7.0, 0.0]]);
        }),
        ("wrong column count", |raw: &mut Value| {
            raw["decision_loss"]["loss"][0] = json!([0.0]);
        }),
        ("duplicate permitted action", |raw: &mut Value| {
            raw["permitted_actions"] = json!(["accept", "accept"]);
        }),
        ("unknown nested field", |raw: &mut Value| {
            raw["decision_loss"]["posterior"] = json!([0.5, 0.5]);
        }),
        ("oversized identifier", |raw: &mut Value| {
            raw["decision_loss"]["models"][0] = json!("x".repeat(257));
        }),
    ];

    for (label, mutate) in cases {
        let mut raw = decision_query();
        mutate(&mut raw);
        assert!(
            Query::from_json(raw).is_err(),
            "malformed contract case {label:?} unexpectedly parsed"
        );
    }
}

#[test]
fn malformed_rate_distortion_bindings_are_refused_before_compilation() {
    let cases: [(&str, QueryMutation); 5] = [
        ("wrong prior shape", |raw: &mut Value| {
            raw["rate_distortion"]["prior"] = json!([1.0]);
        }),
        ("negative compatibility floor", |raw: &mut Value| {
            raw["rate_distortion"]["compatibility_floor"] = json!(-0.1);
        }),
        ("wrong likelihood shape", |raw: &mut Value| {
            raw["rate_distortion"]["evidence_pool"]["items"][0]["likelihood"] = json!([1.0]);
        }),
        ("duplicate evidence id", |raw: &mut Value| {
            raw["rate_distortion"]["evidence_pool"]["items"][1]["id"] = json!("scanner-review");
        }),
        ("unknown rate-distortion field", |raw: &mut Value| {
            raw["rate_distortion"]["posterior"] = json!([0.5, 0.25, 0.25]);
        }),
    ];

    for (label, mutate) in cases {
        let mut raw = rate_distortion_query();
        mutate(&mut raw);
        assert!(
            Query::from_json(raw).is_err(),
            "malformed rate-distortion case {label:?} unexpectedly parsed"
        );
    }
}

#[test]
fn the_published_schema_names_the_same_required_contract_boundary() {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("schemas/fiber-v0.3/query.schema.json"),
        )
        .expect("0.3 schema is readable"),
    )
    .expect("0.3 schema is valid JSON");

    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        json!("fiber-query/0.3")
    );
    assert_eq!(
        schema["properties"]["decision_loss"]["properties"]["loss"]["maxItems"],
        json!(1000)
    );
    assert_eq!(
        schema["properties"]["permitted_actions"]["maxItems"],
        json!(1000)
    );
    let required = schema["required"].as_array().expect("required is an array");
    assert!(required.iter().any(|item| item == "decision_loss"));
    assert!(required.iter().any(|item| item == "permitted_actions"));
}

#[test]
fn the_published_v04_schema_names_the_rate_distortion_boundary() {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("schemas/fiber-v0.4/query.schema.json"),
        )
        .expect("0.4 schema is readable"),
    )
    .expect("0.4 schema is valid JSON");

    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        json!("fiber-query/0.4")
    );
    assert_eq!(
        schema["properties"]["rate_distortion"]["properties"]["evidence_pool"]["properties"]
            ["items"]["maxItems"],
        json!(16)
    );
    let required = schema["required"].as_array().expect("required is an array");
    for field in ["distortion_tolerance", "rate_distortion"] {
        assert!(
            required.iter().any(|item| item == field),
            "{field} required"
        );
    }
}
