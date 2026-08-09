//! The policy pass (43.33) and its collision with mandatory closure (43.13).
//!
//! Two of these tests read `reference/fiber_runtime/examples/policy_restricted_*.json` rather than
//! building a world inline. That is deliberate and is the parity discipline applied to the new
//! pass: the CPython reference compiles those same bytes in
//! `reference/fiber_runtime/tests/test_fiber_runtime.py` and asserts the same two digests, so a
//! change to either implementation's policy handling breaks one suite or the other. The shipped
//! `fixtures/fiber-v0.1` pair is untouched because the reference world declares no policy
//! requirement, and its certificate digest is unmoved.

use bioprism_fiber::{compile, FiberError, PolicyViolation, Query};
use bioprism_section::{CertificateProfile, InfluenceClass, OracleStatus, UnresolvedObligation};
use bioprism_world::World;
use serde_json::{json, Value};
use std::path::PathBuf;

/// The certificate the CPython reference produces for the restricted world under `research-only`.
///
/// Asserted here and in the Python suite. Two literals, one number: if they ever disagree a
/// certificate written by one implementation stops replaying against the other.
const POLICY_RESTRICTED_CERTIFICATE: &str =
    "7c26ed5dee031c10b5433cb266835e8341d5f50497a8cc56ea6fc029ee90d097";
const POLICY_RESTRICTED_SECTION: &str =
    "309ea233223cb6242d495efddb98861d3bc58ff0d024f58029b8e315bce490ae";

fn reference_example(name: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "reference",
        "fiber_runtime",
        "examples",
        name,
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing reference example {}: {e}", path.display()));
    serde_json::from_str(&text).expect("reference example is valid JSON")
}

fn restricted_world() -> World {
    World::from_json(reference_example("policy_restricted_world.json")).expect("world loads")
}

fn restricted_query() -> Query {
    Query::from_json(reference_example("policy_restricted_query.json")).expect("query loads")
}

/// The restricted query with its accepted clauses replaced.
fn with_clauses(clauses: &[&str]) -> Query {
    let mut raw = reference_example("policy_restricted_query.json");
    raw["policy"] = json!(clauses);
    Query::from_json(raw).expect("query loads")
}

/// A world whose single non-protected fact is released only under `clause`.
///
/// Built inline rather than shipped, because each variation below changes one field and a file per
/// variation would hide which field.
fn world_with_policy_scope(scope_policy: Value) -> World {
    World::from_json(json!({
        "schema_version": "fiber-world/0.1",
        "world_id": "policy-variation-v1",
        "events": [],
        "factors": [{
            "id": "factor.check",
            "inputs": ["restricted_reading", "cohort_id"],
            "outputs": ["split_integrity_status"],
            "kind": "rule",
            "scope": {"cohort": "PV-001"}
        }],
        "facts": [
            {
                "id": "fact.cohort",
                "provides": "cohort_id",
                "value": "PV-001",
                "scope": {"cohort": "PV-001"},
                "tags": ["protected"]
            },
            {
                "id": "fact.policy",
                "provides": "data_policy",
                "value": ["research-only", "no-identifiable-export"],
                "scope": {"cohort": "PV-001"},
                "tags": ["protected"]
            },
            {
                "id": "fact.restricted",
                "provides": "restricted_reading",
                "value": 1.0,
                "scope": {"cohort": "PV-001", "policy": scope_policy},
                "tags": []
            }
        ]
    }))
    .expect("variation world loads")
}

fn variation_query(policy: &[&str]) -> Query {
    Query::from_json(json!({
        "schema_version": "fiber-query/0.1",
        "query_id": "variation-v1",
        "targets": ["split_integrity_status"],
        "protected_tags": ["protected"],
        "decision_time": "2025-01-01T00:00:00Z",
        "budgets": {"max_facts": 64},
        "policy": policy
    }))
    .expect("variation query loads")
}

/// 43.13's closure is mandatory, so policy removing a member of it is not a smaller compile.
///
/// The compile refuses. A section built from a policy-truncated closure would present a complete
/// receipt over an incomplete basis, and `fiber-context-certificate/0.1` has no field in which to
/// say otherwise — the exact "guessed correctly from evidence it never had" state the temporal cut
/// is allowed to survive only because it *can* name its exclusions on the wire.
#[test]
fn a_protected_fact_made_inaccessible_by_policy_voids_the_sufficiency_claim() {
    let query =
        Query::from_json(reference_example("policy_protected_closure_query.json")).expect("loads");
    match compile(&restricted_world(), &query) {
        Err(FiberError::Policy(PolicyViolation::ProtectedClosureWithheld {
            fact_ids,
            clauses,
        })) => {
            assert_eq!(fact_ids, vec!["fact.subject_aliases"]);
            assert_eq!(clauses, vec!["no-identifiable-export"]);
        }
        other => panic!("a policy-truncated closure must refuse, got {other:?}"),
    }
}

/// The same fact, the same clause, the same world — protected or not is the only difference.
///
/// Pairing this with the test above is what makes the refusal attributable to closure membership
/// rather than to the policy screen refusing everything it touches.
#[test]
fn the_same_withholding_compiles_when_the_fact_is_not_in_the_protected_closure() {
    let out = compile(&restricted_world(), &restricted_query()).expect("compiles");
    assert_eq!(out.trace.policy.withheld, vec!["fact.subject_aliases"]);
    assert!(out.protected_closure_satisfied());
    assert!(!out.policy_released_everything());
}

/// A policy omission is its own influence class and shares no group with any other.
#[test]
fn a_policy_withheld_fact_is_classified_inaccessible_by_policy_and_not_merged() {
    let out = compile(&restricted_world(), &restricted_query()).expect("compiles");
    let manifest = &out.certificate.manifest;

    assert_eq!(manifest.count_in(InfluenceClass::InaccessibleByPolicy), 1);
    assert_eq!(manifest.count_in(InfluenceClass::DeferredAcquisition), 0);
    assert_eq!(manifest.count_in(InfluenceClass::Zero), 0);
    assert_eq!(manifest.count_in(InfluenceClass::Unknown), 0);
    assert_eq!(manifest.total_omitted(), 1);

    let group = manifest
        .groups
        .iter()
        .find(|g| g.influence == InfluenceClass::InaccessibleByPolicy)
        .expect("the policy group exists");
    assert_eq!(group.examples, vec!["fact.subject_aliases"]);
    assert_eq!(group.bound, None, "no bound was computed, so none is claimed");
}

/// The oracle answers `valid`; the certificate refuses to call that answer sufficient.
///
/// This is the honest-labelling product in one assertion. The withheld fact is the alias table
/// that would have produced an `identity_leakage` witness, so the verdict is not merely
/// under-evidenced, it is *wrong* — and the receipt says the basis does not support a claim.
#[test]
fn an_oracle_verdict_reached_without_policy_withheld_evidence_is_not_sufficient() {
    let out = compile(&restricted_world(), &restricted_query()).expect("compiles");

    assert_eq!(out.certificate.oracle.status, OracleStatus::Valid);
    assert!(out.certificate.oracle.witnesses.is_empty());
    assert!(
        !out.certificate.manifest.supports_sufficiency_claim(),
        "a policy-blocked omission can never support a sufficiency claim"
    );

    let extended = out
        .certificate
        .to_json(CertificateProfile::Extended)
        .expect("extended profile serialises");
    assert_eq!(extended["supports_sufficiency_claim"], json!(false));
}

/// Accepting the obligation is what unlocks the evidence, and the verdict flips when it does.
#[test]
fn accepting_more_obligations_releases_more_evidence_and_changes_the_verdict() {
    let world = restricted_world();
    let permitted = compile(&world, &with_clauses(&["research-only", "no-identifiable-export"]))
        .expect("compiles");

    assert!(permitted.trace.policy.withheld.is_empty());
    assert_eq!(permitted.certificate.selected_facts.len(), 4);
    assert_eq!(permitted.certificate.oracle.status, OracleStatus::Invalid);
    assert_eq!(
        permitted.certificate.oracle.witness_kinds(),
        vec!["identity_leakage"]
    );
    assert!(permitted.certificate.manifest.supports_sufficiency_claim());
}

/// 40.25's named `policy conflict` failure, with a firing condition at last.
#[test]
fn a_clause_the_corpus_never_granted_is_a_policy_conflict_not_a_silent_grant() {
    match compile(&restricted_world(), &with_clauses(&["research-only", "commercial-use"])) {
        Err(FiberError::Policy(PolicyViolation::Conflict { clauses, governing })) => {
            assert_eq!(clauses, vec!["commercial-use"]);
            assert_eq!(governing, vec!["no-identifiable-export", "research-only"]);
        }
        other => panic!("an ungranted clause must refuse, got {other:?}"),
    }
}

/// The conflict gate runs before any selection, so it wins against a failure the passes raise.
///
/// A budget of one cannot hold this world's closure, so `BudgetExceeded` is waiting downstream.
/// Seeing the conflict instead is the observable consequence of gating before the closure.
#[test]
fn a_policy_conflict_is_raised_before_any_evidence_is_selected() {
    let mut raw = reference_example("policy_restricted_query.json");
    raw["policy"] = json!(["commercial-use"]);
    raw["budgets"]["max_facts"] = json!(1);
    let query = Query::from_json(raw).expect("query loads");

    assert!(matches!(
        compile(&restricted_world(), &query),
        Err(FiberError::Policy(PolicyViolation::Conflict { .. }))
    ));
}

/// A world that declares no data policy cannot corroborate the caller, and the trace says so.
///
/// It is not a conflict — there is no authority to conflict with — and it is not a clean bill of
/// health either. "Nobody checked" gets its own field rather than being folded into either.
#[test]
fn a_world_declaring_no_data_policy_records_the_caller_clauses_as_unverified() {
    let world = World::from_json(json!({
        "schema_version": "fiber-world/0.1",
        "world_id": "no-data-policy-v1",
        "events": [],
        "factors": [{
            "id": "factor.check",
            "inputs": ["cohort_id"],
            "outputs": ["split_integrity_status"],
            "kind": "rule",
            "scope": {}
        }],
        "facts": [{
            "id": "fact.cohort",
            "provides": "cohort_id",
            "value": "ND-001",
            "scope": {},
            "tags": ["protected"]
        }]
    }))
    .expect("world loads");

    let out = compile(&world, &variation_query(&["research-only"])).expect("compiles");
    assert_eq!(out.trace.policy.governing, None);
    assert_eq!(out.trace.policy.in_force, vec!["research-only"]);
    assert_eq!(out.trace.policy.unverified, vec!["research-only"]);
    assert!(out.trace.policy.unaccepted.is_empty());
}

/// Governing clauses the caller declined are reported, because they are why evidence is missing.
#[test]
fn governing_clauses_the_caller_declined_are_named_on_the_trace() {
    let out = compile(&restricted_world(), &restricted_query()).expect("compiles");
    assert_eq!(out.trace.policy.governing.as_deref(), Some(&["no-identifiable-export".to_string(), "research-only".to_string()][..]));
    assert_eq!(out.trace.policy.unaccepted, vec!["no-identifiable-export"]);
    assert!(out.trace.policy.unverified.is_empty());
    assert_eq!(out.trace.policy.requirements_seen, 1);
}

/// A list of clauses is a conjunction: holding one of two is holding neither enough.
#[test]
fn a_fact_requiring_two_clauses_is_released_only_to_a_caller_holding_both() {
    let world = world_with_policy_scope(json!(["no-identifiable-export", "research-only"]));

    let partial = compile(&world, &variation_query(&["research-only"])).expect("compiles");
    assert_eq!(partial.trace.policy.withheld, vec!["fact.restricted"]);

    let full = compile(
        &world,
        &variation_query(&["no-identifiable-export", "research-only"]),
    )
    .expect("compiles");
    assert!(full.trace.policy.withheld.is_empty());
}

/// An empty binding names no clause, and is refused rather than read as "requires nothing".
#[test]
fn a_policy_binding_that_names_no_clause_is_refused_rather_than_read_as_unconstrained() {
    for empty in [json!([]), json!("")] {
        let world = world_with_policy_scope(empty.clone());
        match compile(&world, &variation_query(&["research-only"])) {
            Err(FiberError::Policy(PolicyViolation::UninterpretableRequirement {
                fact_id,
                ..
            })) => assert_eq!(fact_id, "fact.restricted"),
            other => panic!("{empty} must be refused, got {other:?}"),
        }
    }
}

/// A malformed governing policy is refused, not treated as an absent one.
///
/// Reading it as absent would silently downgrade a corrupt authority into an unchecked compile,
/// which is the difference between a failure and a wrong answer.
#[test]
fn a_data_policy_that_is_not_a_clause_list_is_refused_rather_than_ignored() {
    let world = World::from_json(json!({
        "schema_version": "fiber-world/0.1",
        "world_id": "malformed-policy-v1",
        "events": [],
        "factors": [{
            "id": "factor.check",
            "inputs": ["cohort_id"],
            "outputs": ["split_integrity_status"],
            "kind": "rule",
            "scope": {}
        }],
        "facts": [
            {"id": "fact.cohort", "provides": "cohort_id", "value": "MP-001", "scope": {}, "tags": ["protected"]},
            {"id": "fact.policy", "provides": "data_policy", "value": {"tier": "gold"}, "scope": {}, "tags": []}
        ]
    }))
    .expect("world loads");

    match compile(&world, &variation_query(&["research-only"])) {
        Err(FiberError::Policy(PolicyViolation::MalformedDataPolicy { fact_id, .. })) => {
            assert_eq!(fact_id, "fact.policy");
        }
        other => panic!("a malformed data policy must refuse, got {other:?}"),
    }
}

/// Policy is the stronger exclusion: a fact excluded by both reasons is reported as policy.
///
/// Reporting it as `DeferredAcquisition` would promise a retry after the cut advances, and the
/// caller would still not hold the clause. The reference certificate's
/// `inaccessible_selected_before_cut` stays empty for the same reason: the cut did not withhold it.
#[test]
fn a_fact_excluded_by_both_policy_and_the_cut_is_reported_as_policy_not_deferred() {
    let world = World::from_json(json!({
        "schema_version": "fiber-world/0.1",
        "world_id": "policy-and-cut-v1",
        "events": [{
            "id": "event.release",
            "event_time": "2025-06-01T00:00:00Z",
            "availability_time": "2025-06-01T00:00:00Z",
            "produces": ["restricted_reading"]
        }],
        "factors": [{
            "id": "factor.check",
            "inputs": ["restricted_reading", "cohort_id"],
            "outputs": ["split_integrity_status"],
            "kind": "rule",
            "scope": {}
        }],
        "facts": [
            {"id": "fact.cohort", "provides": "cohort_id", "value": "PC-001", "scope": {}, "tags": ["protected"]},
            {"id": "fact.policy", "provides": "data_policy", "value": ["research-only", "no-identifiable-export"], "scope": {}, "tags": ["protected"]},
            {"id": "fact.restricted", "provides": "restricted_reading", "value": 1.0, "scope": {"policy": "no-identifiable-export"}, "tags": []}
        ]
    }))
    .expect("world loads");

    let out = compile(&world, &variation_query(&["research-only"])).expect("compiles");
    assert_eq!(out.trace.policy.withheld, vec!["fact.restricted"]);
    assert!(out.certificate.omissions.inaccessible_selected_before_cut.is_empty());
    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::InaccessibleByPolicy),
        1
    );
    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::DeferredAcquisition),
        0
    );
}

/// The screen removes evidence, not structure.
///
/// The factor whose input was withheld stays in the plan, so the receipt still shows what the
/// decision needed rather than quietly shrinking to what it got.
#[test]
fn the_policy_screen_withholds_evidence_and_leaves_the_compiled_structure_intact() {
    let out = compile(&restricted_world(), &restricted_query()).expect("compiles");
    assert_eq!(
        out.certificate.selected_factors,
        vec!["factor.claim_support", "factor.identity_check"]
    );
    assert_eq!(out.certificate.plan.compiled_factor_count, 2);
    assert_eq!(out.certificate.plan.compiled_fact_count, 3);
    assert!(!out.certificate.selected_facts.contains(&"fact.subject_aliases".to_string()));
}

/// Every withheld fact is named twice: once as an obligation, once as a refinement.
#[test]
fn a_policy_withheld_fact_is_named_in_an_obligation_and_in_the_refinement_frontier() {
    let out = compile(&restricted_world(), &restricted_query()).expect("compiles");

    assert_eq!(
        out.section.unresolved_obligations,
        vec![UnresolvedObligation::PolicyBlocked {
            detail: "fact.subject_aliases requires undeclared policy clauses: no-identifiable-export"
                .into()
        }]
    );
    assert!(out.section.requires_refinement());
    assert_eq!(out.section.refinement_frontier.len(), 1);
    assert_eq!(
        out.section.refinement_frontier[0].action,
        "declare_the_required_policy_clauses_or_obtain_a_grant"
    );
    assert_eq!(
        out.section.refinement_frontier[0].facts,
        vec!["fact.subject_aliases"]
    );
}

/// Pass order is normative: closure, then slice, then policy, then the cut.
///
/// Policy after the closure is what makes a closure collision observable; policy before the cut is
/// what stops a permanently-forbidden fact being reported as merely not-yet-available.
#[test]
fn the_policy_pass_runs_after_the_closure_and_before_the_temporal_cut() {
    let out = compile(&restricted_world(), &restricted_query()).expect("compiles");
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

    let policy = out.trace.passes.iter().find(|p| p.name == "policy").unwrap();
    assert_eq!(policy.retained, 3);
    assert_eq!(
        policy.note,
        "1 clause(s) in force, 1 candidate(s) declared a requirement, 1 withheld"
    );
}

/// Both implementations compile the restricted world to the same bytes.
///
/// The Python suite asserts these same two digests against the same files. Without this the policy
/// pass would be the one part of the compiler with no cross-language check.
#[test]
fn the_engine_and_the_cpython_reference_agree_on_the_restricted_world_digest() {
    let out = compile(&restricted_world(), &restricted_query()).expect("compiles");
    assert_eq!(
        out.certificate
            .digest(CertificateProfile::Reference)
            .unwrap()
            .as_str(),
        POLICY_RESTRICTED_CERTIFICATE
    );
    assert_eq!(
        out.certificate.source_hashes.decision_section_sha256,
        POLICY_RESTRICTED_SECTION
    );
}

/// The shipped reference world is untouched by the new pass, and that is why no digest moved.
///
/// Not a restatement of the parity test next door: this one states the *reason*. No fact in the
/// reference world binds the policy scope dimension, and the query's single clause is a subset of
/// what the corpus was released under, so the pass runs, finds nothing to enforce, and removes
/// nothing.
#[test]
fn the_reference_world_declares_no_policy_requirement_so_the_pass_removes_nothing() {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "fiber-v0.1"]
        .iter()
        .collect();
    let world = World::from_json(
        serde_json::from_str(&std::fs::read_to_string(path.join("radiogenomic_world.json")).unwrap())
            .unwrap(),
    )
    .expect("world loads");
    let query = Query::from_json(
        serde_json::from_str(&std::fs::read_to_string(path.join("leakage_query.json")).unwrap())
            .unwrap(),
    )
    .expect("query loads");

    let out = compile(&world, &query).expect("compiles");
    assert_eq!(out.trace.policy.requirements_seen, 0);
    assert!(out.trace.policy.withheld.is_empty());
    assert_eq!(out.trace.policy.in_force, vec!["research-only"]);
    assert_eq!(
        out.trace.policy.governing.as_deref(),
        Some(&["no-identifiable-export".to_string(), "research-only".to_string()][..]),
        "the reference query accepts a strict subset of what the corpus grants"
    );
    assert_eq!(out.trace.policy.unaccepted, vec!["no-identifiable-export"]);
    assert_eq!(
        out.certificate
            .digest(CertificateProfile::Reference)
            .unwrap()
            .as_str(),
        "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4"
    );
}
