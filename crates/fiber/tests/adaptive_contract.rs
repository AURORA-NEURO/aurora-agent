//! Executable `fiber-query/0.5` coverage for the adaptive acquisition boundary.
//!
//! The test uses the published JSON fixture so parser validation, exact kernel planning,
//! certificate identity, and the explicit no-execution posture all share one artifact.

use bioprism_epistemic::{adaptive::AdaptiveNode, ScriptedExecutor};
use bioprism_fiber::{compile, FiberError, Query, QUERY_ADAPTIVE_FIELD_PATHS};
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

fn adaptive_query() -> Value {
    fixture("fixtures/fiber-v0.5/adaptive_acquisition_query.json")
}

fn reference_world() -> World {
    World::from_json(fixture("fixtures/fiber-v0.1/radiogenomic_world.json"))
        .expect("reference world loads")
}

#[test]
fn version_05_executes_an_exact_policy_and_keeps_execution_unstarted() {
    let query = Query::from_json(adaptive_query()).expect("0.5 query parses");
    assert!(query.has_adaptive_acquisition_contract());
    let contract = query
        .adaptive_acquisition
        .as_ref()
        .expect("adaptive contract is present");
    assert_eq!(contract.acquisitions.len(), 2);
    assert_eq!(contract.max_steps, 2);
    assert_eq!(contract.prior.masses().len(), 3);

    let out = compile(&reference_world(), &query).expect("0.5 query compiles");
    let report = out
        .trace
        .adaptive_acquisition
        .as_ref()
        .expect("adaptive policy is in the compiler trace");
    assert_eq!(report.problem.action_count(), 3);
    assert_eq!(report.acquisitions.len(), 2);
    assert!(report.policy.nodes_evaluated > 0);
    assert!(report.policy.expected_total.is_finite());
    assert!(report.policy.selected_depth <= 2);
    assert!(matches!(
        report.policy.root,
        AdaptiveNode::Stop { .. } | AdaptiveNode::Acquire { .. }
    ));
    assert!(out
        .trace
        .passes
        .iter()
        .any(|pass| pass.name == "adaptive_acquisition"));
    assert!(!out
        .trace
        .deferred_passes
        .iter()
        .any(|(name, _)| *name == "adaptive_acquisition"));
    assert_eq!(
        out.certificate
            .digest(CertificateProfile::Reference)
            .expect("certificate digest")
            .as_str()
            .len(),
        64
    );
}

#[test]
fn adaptive_wire_inputs_are_certificate_bound_and_caps_fail_closed() {
    let world = reference_world();
    let first = Query::from_json(adaptive_query()).expect("first query parses");
    let first_out = compile(&world, &first).expect("first query compiles");

    let mut changed = adaptive_query();
    changed["adaptive_acquisition"]["acquisitions"][0]["cost"] = json!(0.3);
    let changed_query = Query::from_json(changed).expect("changed query parses");
    let changed_out = compile(&world, &changed_query).expect("changed query compiles");
    assert_ne!(
        first_out.certificate.source_hashes.query_sha256,
        changed_out.certificate.source_hashes.query_sha256
    );

    let mut over_horizon = adaptive_query();
    over_horizon["adaptive_acquisition"]["max_steps"] = json!(17);
    assert!(matches!(
        Query::from_json(over_horizon),
        Err(FiberError::InvalidAdaptiveAcquisitionContract(_))
    ));

    let mut duplicate_outcome = adaptive_query();
    duplicate_outcome["adaptive_acquisition"]["acquisitions"][0]["outcomes"][1]["label"] =
        json!("positive");
    assert!(matches!(
        Query::from_json(duplicate_outcome),
        Err(FiberError::InvalidAdaptiveAcquisitionContract(_))
    ));
}

#[test]
fn adaptive_field_catalogue_is_version_specific() {
    assert!(QUERY_ADAPTIVE_FIELD_PATHS.contains(&"adaptive_acquisition"));
    assert!(QUERY_ADAPTIVE_FIELD_PATHS.contains(&"adaptive_acquisition.acquisitions"));
    assert!(QUERY_ADAPTIVE_FIELD_PATHS.contains(&"adaptive_acquisition.prior"));
}

#[test]
fn compiled_policy_rebinds_to_execution_and_replays_without_live_fallback() {
    let query = Query::from_json(adaptive_query()).expect("0.5 query parses");
    let out = compile(&reference_world(), &query).expect("0.5 query compiles");
    let trace = out
        .trace
        .adaptive_acquisition
        .as_ref()
        .expect("adaptive trace is present");
    let plan = trace
        .execution_plan()
        .expect("trace is a valid execution plan");
    let digest = plan.digest().expect("plan digest");
    let grant =
        bioprism_epistemic::ExecutionGrant::issue("fiber-test-grant", &digest, "fiber-test")
            .expect("grant scopes to the compiled plan");
    let mut executor = ScriptedExecutor::simulated(
        "fiber-test",
        vec![
            ("screen".into(), "negative".into()),
            ("confirm".into(), "negative".into()),
        ],
    );
    let receipt = plan
        .execute(Some(&grant), &mut executor)
        .expect("simulated execution succeeds");
    assert!(receipt.is_completed());
    // This fixture's exact optimum is an immediate stop, so the valid receipt has no acquisition
    // rows. The lower-level epistemic tests cover a non-empty simulated branch; this integration
    // test proves the compiler-produced stop policy still crosses the execution boundary.
    assert_eq!(receipt.provenance_counts(), (0, 0, 0));
    let replay = plan.replay(&receipt).expect("receipt-only replay succeeds");
    assert!(replay.is_completed());
    assert_eq!(replay.provenance_counts(), (0, 0, 0));
}
