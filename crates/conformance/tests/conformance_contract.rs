//! The invariants this crate is supposed to hold.
//!
//! Blueprint 40.31's verification plan asks for "self-tests for harness" and "intentional failing
//! fixtures", and that is what most of this file is. A conformance runner that only ever sees a
//! conforming implementation is untested where it matters: the interesting question is not
//! whether it says yes to the reference engine, but whether it says no — and says *why* — to an
//! engine that is nondeterministic, silent about its own passes, or quietly wrong.
//!
//! So the deliberately-broken implementations below are the point. Each one breaks exactly one
//! property and the corresponding test asserts that the runner localises it.

use bioprism_conformance::fiber_suite::{
    fiber_suite, shipped_baseline, workspace_fixture_root, FiberReference,
};
use bioprism_conformance::{
    artifact, assess, structural_profile, CaseOutcome, CompileArtifacts, CompileFailure,
    ConformanceCertificate, ConformanceError, FixtureCard, FixtureDrift, FixtureManifest,
    FixtureRole, FixtureShape, FixtureStore, Implementation, ImplementationIdentity,
    InsertObstacle, Layer, Override, PyramidShape, ReleaseGate, Requirement, ShapeChange, Suite,
    SuiteReport,
};
use bioprism_worldgen::{generate, WorldSpec};
use serde_json::{json, Value};
use std::cell::Cell;
use std::path::PathBuf;

// ---------------------------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------------------------

fn fixtures() -> FixtureStore {
    let suite = fiber_suite();
    FixtureStore::load(workspace_fixture_root(), &suite.manifest).expect("fixtures load")
}

fn reference_report() -> SuiteReport {
    let suite = fiber_suite();
    suite
        .run(&FiberReference, &fixtures())
        .expect("the suite runs")
}

fn certified_report() -> SuiteReport {
    reference_report().declaring_baseline(shipped_baseline())
}

fn golden_artifacts() -> CompileArtifacts {
    let store = fixtures();
    FiberReference
        .compile(
            store.get("fiber.world.radiogenomic").unwrap(),
            store.get("fiber.query.leakage").unwrap(),
        )
        .expect("the golden pair compiles")
}

/// Publishes everything the reference does except its own compiler report.
struct Silent;

impl Implementation for Silent {
    fn identity(&self) -> ImplementationIdentity {
        ImplementationIdentity::new("silent-compiler", "0.0.1", "rust")
    }

    fn compile(&self, world: &Value, query: &Value) -> Result<CompileArtifacts, CompileFailure> {
        let mut artifacts = FiberReference.compile(world, query)?;
        artifacts.artifacts.remove(artifact::REPORT);
        Ok(artifacts)
    }
}

/// Emits a different section on every call.
struct Nondeterministic {
    calls: Cell<u64>,
}

impl Implementation for Nondeterministic {
    fn identity(&self) -> ImplementationIdentity {
        ImplementationIdentity::new("nondeterministic-compiler", "0.0.1", "rust")
    }

    fn compile(&self, world: &Value, query: &Value) -> Result<CompileArtifacts, CompileFailure> {
        let nonce = self.calls.get();
        self.calls.set(nonce + 1);
        let mut artifacts = FiberReference.compile(world, query)?;
        if let Some(section) = artifacts.artifacts.get_mut(artifact::SECTION) {
            section["run_nonce"] = json!(nonce);
        }
        Ok(artifacts)
    }
}

/// Never refuses anything: replays the golden artifacts whatever it is given.
struct NeverRefuses {
    artifacts: CompileArtifacts,
}

impl Implementation for NeverRefuses {
    fn identity(&self) -> ImplementationIdentity {
        ImplementationIdentity::new("never-refuses", "0.0.1", "rust")
    }

    fn compile(&self, _world: &Value, _query: &Value) -> Result<CompileArtifacts, CompileFailure> {
        Ok(self.artifacts.clone())
    }
}

/// Refuses exactly what the reference refuses, and says only that something was wrong.
struct VagueRefusal;

impl Implementation for VagueRefusal {
    fn identity(&self) -> ImplementationIdentity {
        ImplementationIdentity::new("vague-refusal", "0.0.1", "rust")
    }

    fn compile(&self, world: &Value, query: &Value) -> Result<CompileArtifacts, CompileFailure> {
        FiberReference
            .compile(world, query)
            .map_err(|failure| CompileFailure::new(failure.kind, "the query is invalid"))
    }
}

/// Drops one protected fact from the certificate it publishes.
struct DropsProtectedEvidence;

impl Implementation for DropsProtectedEvidence {
    fn identity(&self) -> ImplementationIdentity {
        ImplementationIdentity::new("drops-protected", "0.0.1", "rust")
    }

    fn compile(&self, world: &Value, query: &Value) -> Result<CompileArtifacts, CompileFailure> {
        let mut artifacts = FiberReference.compile(world, query)?;
        if let Some(certificate) = artifacts.artifacts.get_mut(artifact::CERTIFICATE) {
            if let Some(facts) = certificate["selected_facts"].as_array_mut() {
                facts.retain(|id| id != "fact.subject_aliases");
            }
        }
        Ok(artifacts)
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("bioprism-conformance-tests")
        .join(format!("{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn tiny_document(facts: usize) -> Value {
    json!({
        "schema_version": "scratch/0.1",
        "name": "tiny",
        "facts": (0..facts).map(|i| json!({ "id": i })).collect::<Vec<Value>>(),
    })
}

/// A one-fixture manifest whose card is generated from `declared`, pointing at a file containing
/// `on_disk`. When the two differ the store must notice.
fn scratch_store(name: &str, declared: &Value, on_disk: &str) -> FixtureStore {
    let root = scratch(name);
    std::fs::write(root.join("doc.json"), on_disk).expect("write fixture");
    let manifest = FixtureManifest::new(
        format!("scratch-{name}"),
        vec![FixtureCard {
            id: "scratch.doc".to_string(),
            path: "doc.json".to_string(),
            role: FixtureRole::World,
            sha256: bioprism_ids::ContentHash::of_value(declared)
                .unwrap()
                .as_str()
                .to_string(),
            provenance: "generated in-test".to_string(),
            license: "Apache-2.0".to_string(),
            synthetic: true,
            shape: FixtureShape::of(declared).unwrap(),
            notes: String::new(),
        }],
    );
    FixtureStore::load(root, &manifest).expect("store loads")
}

// ---------------------------------------------------------------------------------------------
// The suite as a specification artifact (40.32)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_reference_implementation_satisfies_every_requirement_in_the_suite() {
    let report = reference_report();
    assert!(
        report.is_fully_conformant(),
        "the reference engine must pass its own published suite:\n{}",
        report.summary()
    );
    assert_eq!(report.failed(), 0);
    assert_eq!(report.unsupported(), 0);
    assert_eq!(report.errored(), 0);
    assert!(report.passed() >= 25, "suite shrank to {}", report.passed());
}

#[test]
fn every_case_names_the_blueprint_module_it_enforces() {
    let suite = fiber_suite();
    for case in &suite.cases {
        assert!(
            !case.enforces.is_empty(),
            "case {} enforces no blueprint module, so it is a regression test rather than a \
             conformance requirement",
            case.id
        );
        assert!(
            !case.invariant.is_empty(),
            "case {} states no invariant",
            case.id
        );
    }
    let modules = suite.enforced_modules();
    for required in ["43.13", "43.16", "43.26", "43.41", "40.33", "40.05"] {
        assert!(
            modules.contains(required),
            "the suite claims to certify a FIBER compiler but enforces nothing from {required}"
        );
    }
}

#[test]
fn the_suite_round_trips_through_json_so_a_third_party_can_run_it() {
    let suite = fiber_suite();
    let document = suite.to_json().expect("suite serialises");
    let restored = Suite::from_json(&document).expect("suite deserialises");
    assert_eq!(
        restored, suite,
        "the suite is not faithfully representable as data"
    );
    assert_eq!(
        restored.digest().unwrap(),
        suite.digest().unwrap(),
        "a round trip changed the suite digest"
    );
}

#[test]
fn the_published_wire_shape_still_reads_for_a_runner_that_predates_inserts() {
    let document = fiber_suite().to_json().expect("suite serialises");
    let cases = document["cases"].as_array().unwrap();
    let case = |id: &str| {
        cases
            .iter()
            .find(|c| c["id"] == json!(id))
            .unwrap_or_else(|| panic!("{id} is published"))
            .clone()
    };

    let inserting = case("fiber.conformance.an_undeclared_query_field_is_refused");
    assert_eq!(
        inserting["input"]["query_overrides"][0]["op"],
        json!("insert")
    );
    assert_eq!(
        inserting["expect"][0]["naming"],
        json!(["decision_loss"]),
        "a third-party runner has to be told which fragment the refusal must quote"
    );

    let replacing = case("fiber.conformance.a_budget_below_the_protected_closure_is_refused");
    assert!(
        replacing["input"]["query_overrides"][0].get("op").is_none(),
        "an override that replaces must keep the two-field shape it has always had"
    );
    assert!(
        replacing["expect"][0].get("naming").is_none(),
        "a refusal asserted by kind alone must not start demanding a message fragment"
    );
}

#[test]
fn a_suite_declaring_an_unknown_schema_version_is_refused() {
    let mut document = fiber_suite().to_json().unwrap();
    document["schema_version"] = json!("bioprism-conformance-suite/9.9");
    assert!(matches!(
        Suite::from_json(&document),
        Err(ConformanceError::MalformedSuite(_))
    ));
}

#[test]
fn changing_one_case_changes_the_suite_digest() {
    let suite = fiber_suite();
    let before = suite.digest().unwrap();

    let mut mutated = suite.clone();
    mutated.cases.pop();
    let after = mutated.digest().unwrap();

    assert_ne!(
        before, after,
        "a suite that drops a requirement must not keep the digest that certified the old set"
    );
    assert_eq!(before.len(), 64);
}

// ---------------------------------------------------------------------------------------------
// Golden fixtures (40.33)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_fixture_whose_digest_drifted_fails_loudly_rather_than_silently_passing() {
    let store = scratch_store(
        "value-drift",
        &tiny_document(3),
        &serde_json::to_string(&tiny_document(4)).unwrap(),
    );

    let error = store
        .verify()
        .expect_err("a drifted fixture must not verify");
    match error {
        ConformanceError::FixtureDrifted { id, detail } => {
            assert_eq!(id, "scratch.doc");
            assert!(
                detail.contains("facts: 3 -> 4"),
                "drift must say what moved, got: {detail}"
            );
        }
        other => panic!("expected typed drift, got {other:?}"),
    }
}

#[test]
fn fixture_drift_names_the_structural_change_not_only_the_digest() {
    let mut declared = tiny_document(2);
    declared["extra"] = json!("removed later");
    let store = scratch_store(
        "shape-drift",
        &declared,
        &serde_json::to_string(&tiny_document(2)).unwrap(),
    );

    let drift = store.drift().first().expect("drift recorded").clone();
    assert!(drift
        .changes
        .contains(&ShapeChange::KeyRemoved("extra".to_string())));
    assert!(
        drift
            .to_string()
            .contains("top-level key \"extra\" removed"),
        "{drift}"
    );
    assert_ne!(drift.declared_sha256, drift.observed_sha256);
}

#[test]
fn an_edit_that_preserves_the_shape_is_still_reported_as_drift() {
    // Same key count, same collection sizes, same canonical length: the shape summary can say
    // nothing at all here, and that is precisely the case where it must not stay quiet.
    let mut edited = tiny_document(2);
    edited["name"] = json!("tyny");
    let store = scratch_store(
        "value-edit",
        &tiny_document(2),
        &serde_json::to_string(&edited).unwrap(),
    );

    let drift = store.drift().first().expect("drift recorded");
    assert!(
        drift.to_string().contains("edited in place"),
        "a same-shape edit must still be called out: {drift}"
    );
}

#[test]
fn reformatting_a_fixture_is_not_drift_because_digests_are_over_canonical_values() {
    let declared = tiny_document(3);
    let reformatted = serde_json::to_string_pretty(&json!({
        "facts": declared["facts"],
        "name": declared["name"],
        "schema_version": declared["schema_version"],
    }))
    .unwrap();

    let store = scratch_store("reformat", &declared, &reformatted);
    assert!(
        store.drift().is_empty(),
        "reindenting and reordering keys changes no expectation and must not be drift"
    );
    store.verify().expect("no drift");
}

#[test]
fn the_shipped_fixtures_still_hash_to_their_declared_digests() {
    let store = fixtures();
    store
        .verify()
        .expect("fixtures/fiber-v0.1 drifted from the manifest in this crate");
    assert_eq!(store.len(), 4);
    assert_eq!(
        store.synthetic_ids().len(),
        4,
        "every shipped fixture is synthetic and must say so (40.33 invariant 2)"
    );
}

#[test]
fn a_case_referring_to_an_unregistered_fixture_errors_rather_than_reading_the_disk() {
    let mut suite = fiber_suite();
    suite.cases[0].input.world = "fiber.world.does_not_exist".to_string();
    let report = suite.run(&FiberReference, &fixtures()).unwrap();
    let result = &report.results[0];
    match &result.outcome {
        CaseOutcome::Errored { reason } => {
            assert!(reason.contains("not registered"), "{reason}")
        }
        other => panic!("expected an errored case, got {other:?}"),
    }
}

#[test]
fn a_case_whose_override_targets_a_missing_location_errors_rather_than_inventing_it() {
    let mut suite = fiber_suite();
    suite.cases[0]
        .input
        .query_overrides
        .push(bioprism_conformance::Override::new(
            "/budgets/max_widgets",
            json!(7),
        ));
    let report = suite.run(&FiberReference, &fixtures()).unwrap();
    match &report.results[0].outcome {
        CaseOutcome::Errored { reason } => assert!(reason.contains("max_widgets"), "{reason}"),
        other => panic!("expected an errored case, got {other:?}"),
    }
}

#[test]
fn an_insert_override_puts_a_key_into_the_document_the_implementation_actually_reads() {
    // A JSON pointer addresses locations that exist, so this is the one edit a replace can never
    // make. Observed through the compiler rather than through the prepared document, because a key
    // written into a copy the implementation never sees would test nothing.
    let mut suite = fiber_suite();
    suite.cases[0]
        .input
        .query_overrides
        .push(Override::inserting("/tenant_hint", json!("acme")));

    let report = suite.run(&FiberReference, &fixtures()).unwrap();
    match &report.results[0].outcome {
        CaseOutcome::Failed { detail, .. } => assert!(
            detail.contains("tenant_hint"),
            "the inserted key must reach the compiler: {detail}"
        ),
        other => panic!("expected the inserted key to provoke a refusal, got {other:?}"),
    }
}

#[test]
fn an_insert_override_the_fixture_cannot_take_errors_and_names_the_obstacle() {
    for (pointer, obstacle) in [
        ("/query_id", InsertObstacle::KeyAlreadyPresent),
        ("/nowhere/deep_key", InsertObstacle::ParentAbsent),
        ("/targets/0/extra", InsertObstacle::ParentIsNotAnObject),
        ("decision_loss", InsertObstacle::NamesNoParentAndKey),
    ] {
        let mut suite = fiber_suite();
        suite.cases[0]
            .input
            .query_overrides
            .push(Override::inserting(pointer, json!(1)));

        let report = suite.run(&FiberReference, &fixtures()).unwrap();
        match &report.results[0].outcome {
            CaseOutcome::Errored { reason } => assert!(
                reason.contains(&obstacle.to_string()) && reason.contains(pointer),
                "inserting {pointer} must say which obstacle stopped it: {reason}"
            ),
            other => panic!("expected {pointer} to error, got {other:?}"),
        }
    }
}

#[test]
fn a_case_that_inserts_a_key_the_fixture_already_has_is_not_silently_a_replace() {
    // The mirror of the missing-pointer error: an insert states that the baseline lacks the key,
    // and if the baseline grew it the variant stopped differing from the document it is measured
    // against. Overwriting it instead would leave a green case testing nothing.
    let mut suite = fiber_suite();
    suite.cases[0]
        .input
        .query_overrides
        .push(Override::inserting("/role", json!("intruder")));

    let report = suite.run(&FiberReference, &fixtures()).unwrap();
    assert!(
        matches!(&report.results[0].outcome, CaseOutcome::Errored { .. }),
        "got {:?}",
        report.results[0].outcome
    );
}

// ---------------------------------------------------------------------------------------------
// The runner distinguishes four outcomes (40.32)
// ---------------------------------------------------------------------------------------------

#[test]
fn an_implementation_that_publishes_no_report_is_unsupported_rather_than_failed() {
    let report = fiber_suite().run(&Silent, &fixtures()).unwrap();
    let result = report
        .result("fiber.conformance.executed_passes_are_receipted_in_order")
        .expect("case present");
    match &result.outcome {
        CaseOutcome::Unsupported { reason, .. } => {
            assert!(reason.contains("compiler_report"), "{reason}")
        }
        other => panic!("a missing optional artifact is partial support, not failure: {other:?}"),
    }
    assert!(
        report.unsupported() >= 1 && report.failed() == 0,
        "silence about internals is not the same as being wrong:\n{}",
        report.summary()
    );
    assert!(
        !report.is_fully_conformant(),
        "an unchecked requirement is not a satisfied one"
    );
}

#[test]
fn a_nondeterministic_implementation_fails_the_determinism_requirement() {
    let implementation = Nondeterministic {
        calls: Cell::new(0),
    };
    let report = fiber_suite().run(&implementation, &fixtures()).unwrap();
    let result = report
        .result("fiber.property.compilation_is_deterministic")
        .expect("case present");
    match &result.outcome {
        CaseOutcome::Failed { detail, .. } => assert!(
            detail.contains("decision_section") && detail.contains("differs"),
            "{detail}"
        ),
        other => panic!("expected a determinism failure, got {other:?}"),
    }
}

#[test]
fn dropping_one_protected_fact_fails_the_closure_requirement_and_names_it() {
    let report = fiber_suite()
        .run(&DropsProtectedEvidence, &fixtures())
        .unwrap();
    let result = report
        .result("fiber.conformance.the_protected_closure_is_retained")
        .expect("case present");
    match &result.outcome {
        CaseOutcome::Failed { detail, .. } => assert!(
            detail.contains("fact.subject_aliases"),
            "the failure must name the dropped evidence: {detail}"
        ),
        other => panic!("expected a protected-closure failure, got {other:?}"),
    }
}

#[test]
fn an_implementation_that_never_refuses_fails_every_negative_requirement() {
    let implementation = NeverRefuses {
        artifacts: golden_artifacts(),
    };
    let report = fiber_suite().run(&implementation, &fixtures()).unwrap();

    for negative in [
        "fiber.conformance.a_budget_below_the_protected_closure_is_refused",
        "fiber.conformance.a_query_on_an_unsupported_schema_is_refused",
        "fiber.conformance.a_world_on_an_unsupported_schema_is_refused",
        "fiber.conformance.an_empty_query_identifier_is_refused",
        "fiber.conformance.an_undeclared_query_field_is_refused",
        "fiber.conformance.an_undeclared_field_inside_budgets_is_refused",
    ] {
        let result = report.result(negative).expect("case present");
        assert!(
            matches!(result.outcome, CaseOutcome::Failed { .. }),
            "{negative} must fail against an implementation that accepts everything, got {:?}",
            result.outcome
        );
    }
    assert!(
        report.failed() >= 6,
        "the runner must report every failure, not stop at the first:\n{}",
        report.summary()
    );
}

#[test]
fn a_refusal_that_names_no_key_fails_the_undeclared_field_requirement() {
    // Right kind, useless message. The kind alone says the document was rejected as a query; only
    // the named key says which of its fields to go and fix, and 40.33 exists so that an
    // implementer is told that rather than left to infer it.
    let report = fiber_suite().run(&VagueRefusal, &fixtures()).unwrap();

    for (case_id, key) in [
        (
            "fiber.conformance.an_undeclared_query_field_is_refused",
            "decision_loss",
        ),
        (
            "fiber.conformance.an_undeclared_field_inside_budgets_is_refused",
            "budgets.max_items",
        ),
    ] {
        match &report.result(case_id).expect("case present").outcome {
            CaseOutcome::Failed { detail, .. } => assert!(
                detail.contains(key),
                "the failure must say which key went unnamed: {detail}"
            ),
            other => panic!("expected {case_id} to fail, got {other:?}"),
        }
    }

    assert!(
        report
            .result("fiber.conformance.a_budget_below_the_protected_closure_is_refused")
            .expect("case present")
            .outcome
            .is_pass(),
        "a case that asserts only a kind must not be dragged down by a vague message:\n{}",
        report.summary()
    );
}

#[test]
fn the_report_records_which_expectation_kinds_a_passing_run_actually_exercised() {
    let report = reference_report();
    for exercised in [
        "fails_with",
        "embedded_digest_verifies",
        "digest_binds",
        "recompilation_is_byte_identical",
        "matches_golden_fixture",
    ] {
        assert!(
            report.covers(exercised),
            "the run claims to be green but never exercised {exercised}"
        );
    }
    assert!(!report.covers("array_projection_never_defined"));
}

// ---------------------------------------------------------------------------------------------
// The test pyramid (40.31)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_shipped_suite_is_a_balanced_pyramid() {
    let shape = fiber_suite().pyramid();
    let balance = shape.balance();
    assert!(
        balance.is_balanced(),
        "the suite this crate publishes is itself unbalanced:\n{}\n{:?}",
        shape.describe(),
        balance.findings()
    );
    for layer in Layer::ALL {
        assert!(shape.count(layer) > 0, "the {layer} layer is empty");
    }
}

#[test]
fn a_suite_that_is_only_end_to_end_is_reported_unbalanced_with_the_thin_layers_named() {
    let shape = PyramidShape::from_layers([Layer::EndToEnd; 6]);
    let balance = shape.balance();

    assert!(!balance.is_balanced());
    let thin = balance.thin_layers();
    assert_eq!(
        thin,
        vec![
            Layer::Unit,
            Layer::Property,
            Layer::Golden,
            Layer::Conformance
        ],
        "an all-end-to-end suite must name every layer it is missing"
    );
    let rendered: Vec<String> = balance.findings().iter().map(ToString::to_string).collect();
    assert!(
        rendered.iter().any(|f| f.contains("dominates")),
        "{rendered:?}"
    );
    assert!(
        rendered.iter().any(|f| f.contains("inverted")),
        "{rendered:?}"
    );
}

#[test]
fn an_inverted_pyramid_is_reported_even_when_no_layer_is_empty() {
    let mut layers = vec![Layer::Unit];
    layers.extend([Layer::Property; 5]);
    layers.extend([Layer::Golden; 5]);
    layers.extend([Layer::Conformance; 5]);
    layers.extend([Layer::EndToEnd; 4]);

    let balance = PyramidShape::from_layers(layers).balance();
    assert!(!balance.is_balanced());
    let rendered: Vec<String> = balance.findings().iter().map(ToString::to_string).collect();
    assert_eq!(
        rendered.len(),
        1,
        "only the inversion is wrong: {rendered:?}"
    );
    assert!(rendered[0].contains("4 end-to-end cases against 1 unit cases"));
}

#[test]
fn an_empty_suite_is_reported_as_having_no_cases_rather_than_as_balanced() {
    let balance = PyramidShape::from_layers([]).balance();
    assert!(!balance.is_balanced());
    assert_eq!(
        balance
            .findings()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["the suite contains no cases".to_string()]
    );
}

// ---------------------------------------------------------------------------------------------
// Conformance profiles against worlds no fixture describes (40.32, 43.39)
// ---------------------------------------------------------------------------------------------

/// Writes a set of documents to a scratch directory and cards them honestly.
///
/// The digests here are computed from the same values that were written, which is a tautology for
/// drift detection — these fixtures are generated per run, not frozen. That is exactly why the
/// profile they back has no golden layer.
fn generated_store(name: &str, documents: &[(&str, &str, &Value)]) -> (PathBuf, FixtureManifest) {
    let root = scratch(name);
    let cards = documents
        .iter()
        .map(|(id, file, value)| {
            std::fs::write(root.join(file), serde_json::to_string(value).unwrap())
                .expect("write generated fixture");
            FixtureCard {
                id: (*id).to_string(),
                path: (*file).to_string(),
                role: FixtureRole::World,
                sha256: bioprism_ids::ContentHash::of_value(value)
                    .unwrap()
                    .as_str()
                    .to_string(),
                provenance: "generated by bioprism-worldgen from a seeded WorldSpec".to_string(),
                license: "Apache-2.0".to_string(),
                synthetic: true,
                shape: FixtureShape::of(value).unwrap(),
                notes: "regenerated per run; not a frozen golden fixture".to_string(),
            }
        })
        .collect();
    (
        root,
        FixtureManifest::new(format!("generated-{name}"), cards),
    )
}

fn generated_profile(name: &str, spec: &WorldSpec) -> (Suite, FixtureStore) {
    let generated = generate(spec);
    let (root, manifest) = generated_store(
        name,
        &[
            ("gen.world", "world.json", &generated.world),
            ("gen.query", "query.json", &generated.query),
        ],
    );
    let store = FixtureStore::load(root, &manifest).expect("generated store loads");
    let suite = structural_profile(
        format!("fiber-structural-profile-{name}"),
        manifest,
        "gen.world",
        "gen.query",
    );
    (suite, store)
}

#[test]
fn the_structural_profile_certifies_a_world_no_golden_fixture_describes() {
    let (suite, store) = generated_profile(
        "discriminating",
        &WorldSpec::discriminating(200).with_world_id("conformance-generated-v1"),
    );
    let report = suite.run(&FiberReference, &store).unwrap();
    assert!(
        report.is_fully_conformant(),
        "the profile must hold on a world it was not written against:\n{}",
        report.summary()
    );
}

#[test]
fn the_structural_profile_holds_across_structurally_different_worlds() {
    for (name, spec) in [
        ("reference-like", WorldSpec::reference_like(50)),
        ("discriminating", WorldSpec::discriminating(50)),
        (
            "clean-split",
            WorldSpec::discriminating(20).with_leakage(Vec::new()),
        ),
    ] {
        let (suite, store) = generated_profile(name, &spec);
        let report = suite.run(&FiberReference, &store).unwrap();
        assert!(
            report.is_fully_conformant(),
            "profile failed on the {name} world, so a requirement it states is really a \
             statement about one fixture:\n{}",
            report.summary()
        );
    }
}

#[test]
fn a_profile_with_no_golden_layer_is_reported_thin_there_rather_than_balanced() {
    let (suite, _) = generated_profile("shape-only", &WorldSpec::reference_like(10));
    let balance = suite.pyramid().balance();

    assert!(!balance.is_balanced());
    assert_eq!(
        balance.thin_layers(),
        vec![Layer::Golden],
        "a generated world has no independently produced expected output, and that is the only \
         thing wrong with the profile: {}",
        suite.pyramid().describe()
    );
}

#[test]
fn a_structural_profile_alone_cannot_clear_the_release_gates() {
    let (suite, store) = generated_profile("release-attempt", &WorldSpec::discriminating(30));
    let report = suite
        .run(&FiberReference, &store)
        .unwrap()
        .declaring_baseline(shipped_baseline());
    let decision = assess(&report);

    assert!(
        !decision.is_release(),
        "green on generated inputs is not a release: {}",
        decision.explain()
    );
    assert!(decision.blocks_on(ReleaseGate::DeterministicFixturesExist));
    assert!(decision.blocks_on(ReleaseGate::TestPyramidIsBalanced));
    assert!(
        !decision.blocks_on(ReleaseGate::RequiredRequirementsPass),
        "every requirement did pass; what is missing is frozen evidence, not correctness"
    );
}

// ---------------------------------------------------------------------------------------------
// Release gates (40.44, 43.40)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_fully_evidenced_reference_run_meets_every_release_gate() {
    let decision = assess(&certified_report());
    assert!(
        decision.is_release(),
        "the reference run should clear every gate:\n{}",
        decision.explain()
    );
}

#[test]
fn release_is_refused_by_name_when_no_equal_engineering_baseline_is_declared() {
    let decision = assess(&reference_report());
    assert!(!decision.is_release());
    assert!(decision.blocks_on(ReleaseGate::EqualEngineeringBaselineExists));
    assert_eq!(
        decision.unmet().len(),
        1,
        "only the missing baseline should block: {}",
        decision.explain()
    );
    let unmet = &decision.unmet()[0];
    assert!(unmet.because.contains("equal-engineering"), "{unmet}");
    assert_eq!(unmet.gate.source(), "43.40 release gate");
}

#[test]
fn a_baseline_that_loses_the_decision_does_not_satisfy_the_comparator_gate() {
    let mut baseline = shipped_baseline();
    baseline.verdict_preserving = false;
    let decision = assess(&reference_report().declaring_baseline(baseline));

    assert!(decision.blocks_on(ReleaseGate::EqualEngineeringBaselineExists));
    assert!(
        decision.unmet()[0]
            .because
            .contains("does not preserve the decision"),
        "{}",
        decision.explain()
    );
}

#[test]
fn release_is_refused_when_a_case_was_retried_to_green() {
    let report = certified_report().noting_retry("fiber.property.compilation_is_deterministic");
    let decision = assess(&report);

    assert!(decision.blocks_on(ReleaseGate::NoFlakyRetryToGreen));
    assert!(decision.unmet().iter().any(|u| u
        .evidence
        .contains(&"fiber.property.compilation_is_deterministic".to_string())));
}

#[test]
fn release_is_refused_on_fixture_drift_even_though_every_case_passed() {
    let mut report = certified_report();
    assert_eq!(report.failed(), 0);
    report.fixture_drift.push(FixtureDrift {
        id: "fiber.world.radiogenomic".to_string(),
        path: "fiber-v0.1/radiogenomic_world.json".to_string(),
        declared_sha256: "a".repeat(64),
        observed_sha256: "b".repeat(64),
        changes: vec![ShapeChange::CollectionResized {
            key: "facts".to_string(),
            declared: 761,
            observed: 760,
        }],
    });

    let decision = assess(&report);
    assert!(!decision.is_release());
    assert!(decision.blocks_on(ReleaseGate::NoFixtureDrift));
    assert!(
        decision.unmet()[0].evidence[0].contains("facts: 761 -> 760"),
        "the block must carry the drift detail: {}",
        decision.explain()
    );
}

#[test]
fn a_run_that_never_recomputed_a_digest_fails_the_provenance_gate() {
    let mut suite = fiber_suite();
    suite
        .cases
        .retain(|case| !case.expectation_kinds().iter().any(|k| k == "digest_binds"));
    let report = suite
        .run(&FiberReference, &fixtures())
        .unwrap()
        .declaring_baseline(shipped_baseline());

    let decision = assess(&report);
    assert!(decision.blocks_on(ReleaseGate::ProvenanceIsReplayable));
    let unmet = decision
        .unmet()
        .iter()
        .find(|u| u.gate == ReleaseGate::ProvenanceIsReplayable)
        .unwrap();
    assert!(
        unmet.evidence.iter().any(|e| e.contains("digest_binds")),
        "{unmet}"
    );
}

#[test]
fn an_unsupported_mandatory_requirement_blocks_release_like_a_failure_does() {
    let report = fiber_suite()
        .run(&Silent, &fixtures())
        .unwrap()
        .declaring_baseline(shipped_baseline());
    let decision = assess(&report);

    assert!(decision.blocks_on(ReleaseGate::RequiredRequirementsPass));
    let unmet = decision
        .unmet()
        .iter()
        .find(|u| u.gate == ReleaseGate::RequiredRequirementsPass)
        .unwrap();
    assert!(
        unmet.evidence.iter().any(|e| e.contains("unsupported")),
        "{unmet}"
    );
}

#[test]
fn every_gate_names_the_blueprint_clause_it_enforces() {
    for gate in ReleaseGate::ALL {
        assert!(
            !gate.source().is_empty(),
            "gate {gate} cites no blueprint clause"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Certification (40.32)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_conformance_certificate_binds_the_suite_digest_and_verifies() {
    let report = certified_report();
    let certificate = report
        .certify("2026-08-08T00:00:00Z", "2027-08-08T00:00:00Z")
        .expect("a fully conformant run certifies");

    assert_eq!(certificate.suite_digest, report.suite_digest);
    assert_eq!(certificate.requirements.len(), report.results.len());
    assert!(certificate
        .requirements
        .iter()
        .all(|r| r.status == "passed"));

    let document = certificate.to_json().unwrap();
    ConformanceCertificate::verify(&document).expect("its own digest verifies");
}

#[test]
fn a_tampered_conformance_certificate_does_not_verify() {
    let mut document = certified_report()
        .certify("2026-08-08T00:00:00Z", "2027-08-08T00:00:00Z")
        .unwrap()
        .to_json()
        .unwrap();
    document["requirements"][0]["status"] = json!("passed-after-review");

    match ConformanceCertificate::verify(&document) {
        Err(ConformanceError::CertificateDigestMismatch {
            claimed,
            recomputed,
        }) => {
            assert_ne!(claimed, recomputed)
        }
        other => panic!("a rewritten status must invalidate the certificate, got {other:?}"),
    }
}

#[test]
fn certification_is_refused_and_names_the_requirement_that_stopped_it() {
    let report = fiber_suite()
        .run(&DropsProtectedEvidence, &fixtures())
        .unwrap();

    let unmet: Vec<&str> = report
        .unmet_requirements()
        .iter()
        .map(|r| r.case_id.as_str())
        .collect();
    assert!(
        unmet.contains(&"fiber.conformance.the_protected_closure_is_retained"),
        "dropping protected evidence must break the closure requirement: {unmet:?}"
    );

    match report.certify("2026-08-08T00:00:00Z", "2027-08-08T00:00:00Z") {
        Err(ConformanceError::CertificationRefused { reason }) => {
            assert!(reason.contains("failed"), "{reason}");
            assert!(
                unmet.iter().any(|id| reason.contains(id)),
                "the refusal must name a requirement that actually failed: {reason}"
            );
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn certification_is_refused_on_fixture_drift_before_any_requirement_is_considered() {
    let mut report = certified_report();
    report.fixture_drift.push(FixtureDrift {
        id: "fiber.query.leakage".to_string(),
        path: "fiber-v0.1/leakage_query.json".to_string(),
        declared_sha256: "c".repeat(64),
        observed_sha256: "d".repeat(64),
        changes: Vec::new(),
    });

    match report.certify("2026-08-08T00:00:00Z", "2027-08-08T00:00:00Z") {
        Err(ConformanceError::CertificationRefused { reason }) => {
            assert!(reason.starts_with("fixture drift"), "{reason}")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn the_bundle_states_that_it_binds_no_implementation_digest() {
    let report = certified_report();
    assert!(
        !report.implementation.is_digest_bound(),
        "an in-process library cannot hash its own artifact; claiming otherwise would be false"
    );
    assert!(
        report
            .environment
            .unattested
            .contains(&"clean_machine_provisioning".to_string()),
        "the environment manifest must disclose what it does not attest"
    );
}

#[test]
fn the_bundle_carries_the_synthetic_qualification_into_the_result() {
    let report = certified_report();
    assert_eq!(report.synthetic_fixtures.len(), 4);
    let document = report.to_json().unwrap();
    assert_eq!(
        document["synthetic_fixtures"].as_array().unwrap().len(),
        4,
        "40.33 invariant 2 has to travel with the report to do any work"
    );
    assert_eq!(document["fixture_manifest_id"], json!("fiber-v0.1"));
}

#[test]
fn a_recommended_requirement_does_not_block_certification() {
    let mut suite = fiber_suite();
    for case in &mut suite.cases {
        if case.id == "fiber.conformance.executed_passes_are_receipted_in_order" {
            case.requirement = Requirement::Should;
        }
    }
    let report = suite.run(&Silent, &fixtures()).unwrap();
    assert!(
        report
            .result("fiber.conformance.executed_passes_are_receipted_in_order")
            .map(|r| !r.outcome.is_pass())
            .unwrap_or(false),
        "the case should still be reported as not passing"
    );
    assert!(
        report
            .unmet_requirements()
            .iter()
            .all(|r| r.requirement == Requirement::Must),
        "a `should` failure must be disclosed but not counted against certification"
    );
}
