//! Claim-per-test coverage of the research runner: request refusals, protocol determinism,
//! every finding-derivation rule on synthetic measurements (tie included, as a required negative
//! finding), dossier digest round-trip and tamper detection, malformed-versus-mismatch digests,
//! report rendering with verbatim limitations and per-figure source digests, and the
//! single-variant observation level.

use bioprism_baseline::{
    Comparison, Judgement, RowRefusal, RowVerdict, StrategyResult, SweepCell, SweepRow, SweepTable,
};
use bioprism_fiber::FiberError;
use bioprism_ids::{to_canonical_string, ContentHash};
use bioprism_mutation::{Diversity, Family, Instance};
use bioprism_prism::{Minimization, Preservation};
use bioprism_research::{
    artifact_record, comparison_findings, minimization_findings, mutation_findings, plan_protocol,
    reference_anchor_finding, render_report, run_research, sweep_findings, verify_dossier, Finding,
    ObservationLevel, ProtocolStep, ResearchError, ResearchRequest, INLINE_ARTIFACT_CAP_BYTES,
    PINNED_REFERENCE_CERTIFICATE_SHA256, REQUIRED_LIMITATIONS,
};
use bioprism_section::OracleStatus;
use bioprism_worldgen::{DistractorAttachment, TagStyle};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn request_of(value: Value) -> ResearchRequest {
    serde_json::from_value(value).expect("test request must validate")
}

fn request_error(value: Value) -> String {
    serde_json::from_value::<ResearchRequest>(value)
        .expect_err("test request must be refused")
        .to_string()
}

fn base_request_value() -> Value {
    json!({
        "research_id": "run-small",
        "question": "Does hub attachment separate the equal-engineering panel?",
        "family": "reference_like",
        "distractor_points": [40],
        "seed": 11,
    })
}

fn small_request() -> ResearchRequest {
    request_of(base_request_value())
}

fn small_dossier() -> &'static Value {
    static DOSSIER: OnceLock<Value> = OnceLock::new();
    DOSSIER.get_or_init(|| run_research(&small_request()).expect("small run completes"))
}

fn full_request() -> ResearchRequest {
    request_of(json!({
        "research_id": "run-full",
        "question": "Under which structures does any strategy separate from the compiler?",
        "family": "discriminating",
        "distractor_points": [40, 120],
        "seed": 11,
        "run_sweep": true,
        "run_mutation": true,
        "run_minimize": true,
    }))
}

fn full_dossier() -> &'static Value {
    static DOSSIER: OnceLock<Value> = OnceLock::new();
    DOSSIER.get_or_init(|| run_research(&full_request()).expect("full run completes"))
}

fn restamp(dossier: &mut Value) {
    let mut without = dossier.clone();
    without
        .as_object_mut()
        .expect("dossier is an object")
        .remove("dossier_sha256");
    let digest = ContentHash::of_value(&without)
        .expect("dossier canonicalises")
        .to_string();
    dossier["dossier_sha256"] = json!(digest);
}

fn artifact<'a>(dossier: &'a Value, name: &str) -> &'a Value {
    dossier["steps"]
        .as_array()
        .expect("steps array")
        .iter()
        .flat_map(|step| step["outputs"].as_array().expect("outputs array"))
        .find(|output| output["name"] == json!(name))
        .unwrap_or_else(|| panic!("dossier must carry artifact {name}"))
}

fn judged(name: &str, facts: usize, preserving: bool, recall: f64) -> StrategyResult {
    StrategyResult {
        name: name.into(),
        method: "synthetic".into(),
        facts_exposed: facts,
        fraction_of_world: facts as f64 / 100.0,
        verdict: RowVerdict::Judged(Judgement {
            status: OracleStatus::Invalid,
            witnesses: vec!["identity_leakage".into()],
            verdict_preserving: preserving,
            missing_witnesses: Vec::new(),
            spurious_witnesses: Vec::new(),
        }),
        protected_recall: recall,
        notes: Vec::new(),
    }
}

fn synthetic_comparison(results: Vec<StrategyResult>) -> Comparison {
    Comparison {
        world_id: "synthetic-w".into(),
        query_id: "synthetic-q".into(),
        total_facts: 100,
        reference_status: OracleStatus::Invalid,
        reference_witnesses: vec!["identity_leakage".into()],
        results,
    }
}

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn by_rule<'a>(findings: &'a [Finding], rule: &str) -> Vec<&'a Finding> {
    findings.iter().filter(|f| f.rule == rule).collect()
}

fn sweep_cell(id: &str, fiber_admissible: bool, baseline_admissible: bool) -> SweepCell {
    SweepCell {
        world_id: id.into(),
        attachment: DistractorAttachment::Hub,
        relay_depth: 0,
        tag_style: TagStyle::Distinct,
        distractors: 50,
        total_facts: 61,
        rows: vec![
            SweepRow {
                strategy: "full-context".into(),
                facts_selected: 61,
                sound: Some(true),
                protected_closure: 1.0,
                admissible: true,
            },
            SweepRow {
                strategy: "khop-5".into(),
                facts_selected: 11,
                sound: Some(baseline_admissible),
                protected_closure: 1.0,
                admissible: baseline_admissible,
            },
            SweepRow {
                strategy: "fiber".into(),
                facts_selected: 11,
                sound: Some(fiber_admissible),
                protected_closure: 1.0,
                admissible: fiber_admissible,
            },
        ],
    }
}

// ---------------------------------------------------------------- request validation

#[test]
fn a_request_with_no_distractor_points_is_refused() {
    let mut value = base_request_value();
    value["distractor_points"] = json!([]);
    assert!(request_error(value).contains("at least one point"));
}

#[test]
fn a_request_with_more_than_six_distractor_points_is_refused() {
    let mut value = base_request_value();
    value["distractor_points"] = json!([1, 2, 3, 4, 5, 6, 7]);
    assert!(request_error(value).contains("exceed the maximum of 6"));
}

#[test]
fn a_distractor_point_beyond_the_per_point_ceiling_is_refused() {
    let mut value = base_request_value();
    value["distractor_points"] = json!([2001]);
    assert!(request_error(value).contains("per-point ceiling of 2000"));
}

#[test]
fn a_repeated_distractor_point_is_refused() {
    let mut value = base_request_value();
    value["distractor_points"] = json!([40, 40]);
    assert!(request_error(value).contains("repeated"));
}

#[test]
fn an_unknown_field_in_the_request_document_is_refused() {
    let mut value = base_request_value();
    value["surprise"] = json!(true);
    assert!(request_error(value).contains("unknown field"));
}

#[test]
fn a_research_id_outside_the_safe_character_set_is_refused() {
    for bad in ["", "has space", "sla/sh", &"x".repeat(65)] {
        let mut value = base_request_value();
        value["research_id"] = json!(bad);
        assert!(
            request_error(value).contains("research_id"),
            "{bad:?} must be refused"
        );
    }
}

#[test]
fn an_empty_question_is_refused() {
    let mut value = base_request_value();
    value["question"] = json!("   ");
    assert!(request_error(value).contains("question must not be empty"));
}

#[test]
fn a_question_beyond_the_byte_cap_is_refused() {
    let mut value = base_request_value();
    value["question"] = json!("q".repeat(4097));
    assert!(request_error(value).contains("cap is 4096 bytes"));
}

#[test]
fn a_valid_request_survives_a_document_round_trip_with_a_stable_digest() {
    let request = small_request();
    let document = serde_json::to_value(&request).expect("request serialises");
    let reparsed: ResearchRequest = serde_json::from_value(document).expect("round-trips");
    assert_eq!(request, reparsed);
    assert_eq!(request.digest().unwrap(), reparsed.digest().unwrap());
    assert!(ContentHash::parse(request.digest().unwrap()).is_ok());
}

// ---------------------------------------------------------------- protocol assembly

#[test]
fn planning_the_same_request_twice_yields_an_identical_protocol() {
    let request = full_request();
    assert_eq!(plan_protocol(&request), plan_protocol(&request));
}

#[test]
fn the_protocol_opens_with_the_reference_anchor_and_walks_points_in_authored_order() {
    let request = request_of(json!({
        "research_id": "order",
        "question": "q?",
        "family": "reference_like",
        "distractor_points": [200, 40],
        "seed": 1,
    }));
    let protocol = plan_protocol(&request);
    assert_eq!(
        protocol.steps,
        vec![
            ProtocolStep::AnchorReferenceFixture,
            ProtocolStep::GenerateWorld { distractors: 200 },
            ProtocolStep::CompileFiber { distractors: 200 },
            ProtocolStep::ComparePanel { distractors: 200 },
            ProtocolStep::GenerateWorld { distractors: 40 },
            ProtocolStep::CompileFiber { distractors: 40 },
            ProtocolStep::ComparePanel { distractors: 40 },
        ]
    );
}

#[test]
fn optional_steps_enter_the_protocol_only_when_requested_and_target_the_first_point() {
    let bare = plan_protocol(&small_request());
    assert!(!bare
        .steps
        .iter()
        .any(|step| matches!(step, ProtocolStep::SweepStructuralGrid)));
    let full = plan_protocol(&full_request());
    let tail: Vec<&ProtocolStep> = full.steps.iter().rev().take(3).collect();
    assert_eq!(
        tail[2],
        &ProtocolStep::SweepStructuralGrid,
        "sweep precedes mutation and minimization"
    );
    assert_eq!(tail[1], &ProtocolStep::MutateBaseWorld { distractors: 40 });
    assert_eq!(
        tail[0],
        &ProtocolStep::MinimizeBaseWorld { distractors: 40 }
    );
}

// ---------------------------------------------------------------- finding derivation rules

#[test]
fn a_baseline_admissible_at_fibers_cost_derives_a_negative_tie_finding() {
    let comparison = synthetic_comparison(vec![
        judged("full-context", 100, true, 1.0),
        judged("khop-5", 11, true, 1.0),
        judged("fiber", 11, true, 1.0),
    ]);
    let findings = comparison_findings(&comparison, DIGEST);
    let ties = by_rule(&findings, "fiber_tied_by_baseline");
    assert_eq!(ties.len(), 1, "a tie must derive its finding");
    assert!(ties[0].negative, "a tie is a required negative finding");
    assert!(ties[0].claim.contains("khop-5 at 11 facts"));
    assert!(by_rule(&findings, "fiber_separated").is_empty());
}

#[test]
fn the_cheapest_admissible_strategy_is_named_with_its_cost_and_world() {
    let comparison = synthetic_comparison(vec![
        judged("full-context", 100, true, 1.0),
        judged("fiber", 11, true, 1.0),
    ]);
    let findings = comparison_findings(&comparison, DIGEST);
    let cheapest = by_rule(&findings, "cheapest_admissible");
    assert_eq!(cheapest.len(), 1);
    assert!(cheapest[0]
        .claim
        .contains("cheapest admissible strategy on world synthetic-w"));
    assert!(cheapest[0].claim.contains("fiber at 11 facts"));
    assert!(!cheapest[0].negative, "fiber winning is not a null result");
}

#[test]
fn a_cheapest_admissible_that_is_not_fiber_is_flagged_negative() {
    let comparison = synthetic_comparison(vec![
        judged("lexical-top11", 9, true, 1.0),
        judged("fiber", 11, true, 1.0),
    ]);
    let findings = comparison_findings(&comparison, DIGEST);
    let cheapest = by_rule(&findings, "cheapest_admissible");
    assert!(cheapest[0].claim.contains("lexical-top11 at 9 facts"));
    assert!(cheapest[0].negative);
}

#[test]
fn a_panel_with_no_admissible_strategy_derives_a_negative_finding() {
    let comparison = synthetic_comparison(vec![
        judged("khop-5", 20, false, 1.0),
        judged("fiber", 11, false, 1.0),
    ]);
    let findings = comparison_findings(&comparison, DIGEST);
    let none = by_rule(&findings, "no_admissible_strategy");
    assert_eq!(none.len(), 1);
    assert!(none[0].negative);
    assert!(by_rule(&findings, "cheapest_admissible").is_empty());
}

#[test]
fn an_inadmissible_fiber_row_derives_a_negative_finding() {
    let comparison = synthetic_comparison(vec![
        judged("khop-5", 20, true, 1.0),
        judged("fiber", 11, true, 0.9),
    ]);
    let findings = comparison_findings(&comparison, DIGEST);
    let inadmissible = by_rule(&findings, "fiber_inadmissible");
    assert_eq!(inadmissible.len(), 1);
    assert!(inadmissible[0].negative);
    assert!(inadmissible[0].claim.contains("protected closure 90%"));
}

#[test]
fn a_baseline_admissible_only_above_fibers_cost_is_separation_not_a_tie() {
    let comparison = synthetic_comparison(vec![
        judged("khop-7", 40, true, 1.0),
        judged("fiber", 11, true, 1.0),
    ]);
    let findings = comparison_findings(&comparison, DIGEST);
    assert!(by_rule(&findings, "fiber_tied_by_baseline").is_empty());
    let separated = by_rule(&findings, "fiber_separated");
    assert_eq!(separated.len(), 1);
    assert!(!separated[0].negative);
    assert!(separated[0].claim.contains("cost of 11 facts or below"));
}

#[test]
fn an_oracle_refused_row_is_recorded_as_neither_sound_nor_unsound() {
    let mut refused = judged("directed-walk", 3, true, 0.2);
    refused.verdict = RowVerdict::Refused(RowRefusal::OracleRefused {
        facts_exposed: 3,
        source: FiberError::QueryNotAnObject,
    });
    let comparison =
        synthetic_comparison(vec![judged("fiber", 11, true, 1.0), refused]);
    let findings = comparison_findings(&comparison, DIGEST);
    let rows = by_rule(&findings, "oracle_refused_row");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].claim.contains("neither sound nor unsound"));
    assert!(!rows[0].negative, "a refusal is not a refutation");
}

#[test]
fn sweep_tie_cells_are_a_required_negative_finding() {
    let table = SweepTable {
        seed: 7,
        cells: vec![
            sweep_cell("tie", true, true),
            sweep_cell("fiber-only", true, false),
            sweep_cell("fiber-out", false, true),
            sweep_cell("nobody", false, false),
        ],
    };
    let findings = sweep_findings(&table, DIGEST);
    let ties = by_rule(&findings, "sweep_ties");
    assert_eq!(ties.len(), 1);
    assert!(ties[0].negative);
    assert!(ties[0].claim.contains("1 of 4 sweep cells"));
    assert!(by_rule(&findings, "sweep_fiber_only")[0]
        .claim
        .contains("only admissible strategy in 1 of 4"));
    assert!(by_rule(&findings, "sweep_fiber_inadmissible")[0]
        .claim
        .contains("2 of 4"));
    assert!(by_rule(&findings, "sweep_none_admissible")[0].negative);
}

#[test]
fn mutation_inflation_above_one_marks_the_yield_finding_negative() {
    let instance = |id: &str, family: &str| Instance {
        id: id.into(),
        parent_id: "p".into(),
        mutation_id: id.into(),
        family: family.into(),
        world_sha256: DIGEST.into(),
        status: "invalid".into(),
        witnesses: vec!["identity_leakage".into()],
    };
    let family = Family {
        parent_id: "p".into(),
        parent_sha256: DIGEST.into(),
        accepted: vec![instance("a", "invariance"), instance("b", "invariance")],
        ..Family::default()
    };
    let inflated = Diversity {
        instances: 2,
        parents: 1,
        families: 1,
        signatures: 1,
        equivalence_classes: 1,
        inflation_ratio: 2.0,
        caveat: "caveat".into(),
    };
    let honest = Diversity {
        equivalence_classes: 2,
        inflation_ratio: 1.0,
        ..inflated.clone()
    };
    let negative = mutation_findings(&family, &inflated, DIGEST, DIGEST);
    assert!(negative[0].negative);
    assert!(negative[0].claim.contains("instance count is not benchmark count"));
    let positive = mutation_findings(&family, &honest, DIGEST, DIGEST);
    assert!(!positive[0].negative);
}

#[test]
fn a_diverged_or_unverifiable_preservation_makes_the_reduction_finding_negative() {
    let minimization = Minimization {
        started_from: 10,
        minimal: vec!["fact.a".into()],
        removed: 9,
        preserved_status: "invalid".into(),
        preserved_witnesses: vec!["identity_leakage".into()],
        evaluations: 11,
        unjudged: Vec::new(),
        guarantee: "1-minimal".into(),
    };
    let preserved = minimization_findings(&minimization, &Preservation::Preserved, DIGEST);
    assert!(!preserved[0].negative);
    let diverged = minimization_findings(
        &minimization,
        &Preservation::Diverged {
            status: "valid".into(),
            witnesses: Vec::new(),
        },
        DIGEST,
    );
    assert!(diverged[0].negative);
    assert!(diverged[0].claim.contains("DIVERGED"));
    let unverifiable = minimization_findings(
        &minimization,
        &Preservation::Unverifiable {
            detail: "refused".into(),
        },
        DIGEST,
    );
    assert!(unverifiable[0].negative);
    assert!(unverifiable[0].claim.contains("unverifiable"));
}

#[test]
fn a_reduction_claim_leads_with_preservation_only_when_the_re_check_preserved_it() {
    let minimization = Minimization {
        started_from: 10,
        minimal: vec!["fact.a".into()],
        removed: 9,
        preserved_status: "invalid".into(),
        preserved_witnesses: vec!["identity_leakage".into()],
        evaluations: 11,
        unjudged: Vec::new(),
        guarantee: "1-minimal".into(),
    };
    let asserts_preservation = "preserves the oracle signature";

    let preserved = minimization_findings(&minimization, &Preservation::Preserved, DIGEST);
    assert!(
        preserved[0].claim.starts_with("a 1-minimal subset of 1 of 10 facts preserves the oracle"),
        "a preserved re-check may state the preservation first: {}",
        preserved[0].claim
    );

    let diverged = minimization_findings(
        &minimization,
        &Preservation::Diverged {
            status: "valid".into(),
            witnesses: vec!["none".into()],
        },
        DIGEST,
    );
    assert!(
        diverged[0].claim.starts_with("the independent re-check DIVERGED"),
        "a diverged re-check must lead the claim, not trail it after a semicolon: {}",
        diverged[0].claim
    );
    assert!(
        !diverged[0].claim.contains(asserts_preservation),
        "the prose may not assert the preservation the re-check contradicted: {}",
        diverged[0].claim
    );

    let unverifiable = minimization_findings(
        &minimization,
        &Preservation::Unverifiable {
            detail: "the oracle refused".into(),
        },
        DIGEST,
    );
    assert!(
        unverifiable[0].claim.starts_with("the independent re-check was unverifiable"),
        "an unchecked reduction must say so first: {}",
        unverifiable[0].claim
    );
    assert!(
        !unverifiable[0].claim.contains(asserts_preservation),
        "unchecked is not checked, and the prose may not claim otherwise: {}",
        unverifiable[0].claim
    );

    for claim in [&preserved[0].claim, &diverged[0].claim, &unverifiable[0].claim] {
        assert!(claim.contains("10"), "every outcome still reports what was reduced: {claim}");
        assert!(claim.contains("11 evaluation"), "and at what search cost: {claim}");
    }
}

#[test]
fn every_derived_finding_is_an_observation_citing_its_artifact_digest() {
    let comparison = synthetic_comparison(vec![
        judged("khop-5", 11, true, 1.0),
        judged("fiber", 11, true, 1.0),
    ]);
    let mut findings = comparison_findings(&comparison, DIGEST);
    findings.push(reference_anchor_finding(
        PINNED_REFERENCE_CERTIFICATE_SHA256,
        DIGEST,
    ));
    for finding in &findings {
        assert_eq!(finding.level, ObservationLevel::Observation);
        assert!(!finding.supported_by.is_empty());
        assert!(finding.supported_by.iter().all(|digest| digest == DIGEST));
    }
}

#[test]
fn the_observation_level_is_the_only_representable_level() {
    fn the_only_level(level: ObservationLevel) -> &'static str {
        match level {
            ObservationLevel::Observation => "observation",
        }
    }
    assert_eq!(the_only_level(ObservationLevel::Observation), "observation");
    assert_eq!(
        serde_json::from_value::<ObservationLevel>(json!("observation")).unwrap(),
        ObservationLevel::Observation
    );
    for other in ["conclusion", "hypothesis", "Observation", "finding"] {
        assert!(
            serde_json::from_value::<ObservationLevel>(json!(other)).is_err(),
            "{other:?} must be unrepresentable"
        );
    }
}

// ---------------------------------------------------------------- the runner and the dossier

#[test]
fn a_research_run_is_deterministic_byte_for_byte() {
    let again = run_research(&small_request()).expect("second run completes");
    assert_eq!(
        to_canonical_string(small_dossier()).unwrap(),
        to_canonical_string(&again).unwrap()
    );
}

#[test]
fn the_dossier_anchors_to_the_pinned_reference_parity_digest() {
    let record = artifact(small_dossier(), "reference-certificate");
    assert_eq!(
        record["artifact"]["certificate_sha256"],
        json!(PINNED_REFERENCE_CERTIFICATE_SHA256)
    );
    let findings = small_dossier()["findings"].as_array().unwrap();
    assert_eq!(findings[0]["rule"], json!("reference_anchor"));
    assert!(findings[0]["claim"]
        .as_str()
        .unwrap()
        .contains(PINNED_REFERENCE_CERTIFICATE_SHA256));
}

#[test]
fn the_dossier_digest_round_trips_and_tampering_is_detected() {
    let verification = verify_dossier(small_dossier()).expect("verifiable");
    assert_eq!(verification["valid"], json!(true), "{verification}");
    assert_eq!(verification["digest_match"], json!(true));
    assert_eq!(verification["request_digest_match"], json!(true));

    let mut tampered = small_dossier().clone();
    tampered["findings"][0]["claim"] = json!("the compiler is always best");
    let verification = verify_dossier(&tampered).expect("still verifiable in shape");
    assert_eq!(verification["digest_match"], json!(false));
    assert_eq!(verification["valid"], json!(false));
}

#[test]
fn a_malformed_dossier_digest_is_distinguished_from_a_mismatch() {
    let mut malformed = small_dossier().clone();
    malformed["dossier_sha256"] = json!("not-64-hex");
    let verification = verify_dossier(&malformed).expect("shape is still a dossier");
    assert_eq!(verification["digest_malformed"], json!(true));
    assert_eq!(verification["digest_match"], json!(false));

    let mut mismatched = small_dossier().clone();
    mismatched["dossier_sha256"] = json!("0".repeat(64));
    let verification = verify_dossier(&mismatched).expect("well-formed wrong digest");
    assert_eq!(verification["digest_malformed"], json!(false));
    assert_eq!(verification["digest_match"], json!(false));
    assert_eq!(verification["valid"], json!(false));
}

#[test]
fn a_wrong_schema_is_an_error_rather_than_an_invalid_verification() {
    let mut wrong = small_dossier().clone();
    wrong["schema"] = json!("bioprism-research/dossier/9.9");
    assert!(matches!(
        verify_dossier(&wrong),
        Err(ResearchError::InvalidDossier { .. })
    ));
    assert!(matches!(
        verify_dossier(&json!([])),
        Err(ResearchError::InvalidDossier { .. })
    ));
}

#[test]
fn every_artifact_record_carries_a_digest_and_an_explicit_inline_decision() {
    for step in full_dossier()["steps"].as_array().unwrap() {
        for output in step["outputs"].as_array().unwrap() {
            let digest = output["sha256"].as_str().expect("sha256 present");
            assert!(ContentHash::parse(digest).is_ok(), "64-hex digest");
            let inlined = output["inlined"].as_bool().expect("inlined present");
            assert_eq!(
                inlined,
                output.get("artifact").is_some(),
                "the artifact key is present exactly when inlined says so"
            );
            assert!(output["canonical_bytes"].as_u64().unwrap() > 0);
        }
    }
}

#[test]
fn an_artifact_over_the_cap_is_recorded_digest_only() {
    let oversized = json!({ "blob": vec!["x".repeat(1000); 200] });
    let recorded = artifact_record("big", &oversized).expect("records");
    assert!(
        to_canonical_string(&oversized).unwrap().len() > INLINE_ARTIFACT_CAP_BYTES,
        "fixture must exceed the cap"
    );
    assert_eq!(recorded.record["inlined"], json!(false));
    assert!(recorded.record.get("artifact").is_none());
    assert!(ContentHash::parse(recorded.record["sha256"].as_str().unwrap()).is_ok());

    let small = json!({ "tiny": true });
    let recorded = artifact_record("small", &small).expect("records");
    assert_eq!(recorded.record["inlined"], json!(true));
    assert_eq!(recorded.record["artifact"], small);
}

#[test]
fn findings_support_digests_reference_artifacts_the_dossier_carries() {
    let verification = verify_dossier(small_dossier()).expect("verifiable");
    assert_eq!(verification["findings_supported"], json!(true));

    let mut tampered = small_dossier().clone();
    tampered["findings"][0]["supported_by"] = json!(["b".repeat(64)]);
    restamp(&mut tampered);
    let verification = verify_dossier(&tampered).expect("verifiable in shape");
    assert_eq!(verification["digest_match"], json!(true));
    assert_eq!(verification["findings_supported"], json!(false));
    assert_eq!(verification["valid"], json!(false));
}

#[test]
fn a_dossier_missing_a_required_limitation_fails_verification_even_restamped() {
    let mut stripped = small_dossier().clone();
    let limitations = stripped["limitations"].as_array_mut().unwrap();
    limitations.retain(|entry| !entry.as_str().unwrap().contains("oracle review"));
    restamp(&mut stripped);
    let verification = verify_dossier(&stripped).expect("verifiable in shape");
    assert_eq!(verification["digest_match"], json!(true));
    assert_eq!(verification["limitations_present"], json!(false));
    assert_eq!(verification["valid"], json!(false));
    assert!(verification["missing_limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry.as_str().unwrap().contains("oracle review")));
}

#[test]
fn the_question_is_recorded_verbatim_and_rendered_verbatim() {
    let question = "Is `x` **separable**?\n```\ninner fence\n```\n| pipe | table |";
    let request = request_of(json!({
        "research_id": "verbatim",
        "question": question,
        "family": "reference_like",
        "distractor_points": [40],
        "seed": 3,
    }));
    let dossier = run_research(&request).expect("runs");
    assert_eq!(dossier["request"]["question"], json!(question));
    let rendered = render_report(&dossier).expect("renders");
    assert!(
        rendered.report_md.contains(question),
        "the question must appear byte-for-byte in the report"
    );
    assert!(rendered
        .report_md
        .contains("it did not interpret the question"));
}

// ---------------------------------------------------------------- the rendered report

#[test]
fn the_report_renders_every_required_limitation_line_verbatim() {
    let rendered = render_report(small_dossier()).expect("renders");
    for limitation in REQUIRED_LIMITATIONS {
        assert!(
            rendered.report_md.contains(limitation),
            "missing limitation: {limitation}"
        );
    }
}

#[test]
fn every_figure_carries_its_source_digest_in_caption_and_footer() {
    let rendered = render_report(full_dossier()).expect("renders");
    let expected = [
        "selection-ratio-reference.svg",
        "omission-accounting-reference.svg",
        "baseline-panel-d40.svg",
        "baseline-panel-d120.svg",
        "sweep-grid.svg",
        "mutation-diversity.svg",
    ];
    assert_eq!(
        rendered
            .figures
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    for (filename, svg) in &rendered.figures {
        assert!(rendered.report_md.contains(filename.as_str()));
        let caption_start = rendered
            .report_md
            .find(filename.as_str())
            .expect("filename in report");
        let caption = &rendered.report_md[caption_start..];
        let digest_line = caption
            .lines()
            .find(|line| line.contains("sha256 `"))
            .expect("caption cites a digest");
        let digest = digest_line
            .split("sha256 `")
            .nth(1)
            .and_then(|rest| rest.split('`').next())
            .expect("digest extractable");
        assert!(ContentHash::parse(digest).is_ok(), "caption digest is 64-hex");
        assert!(
            svg.contains(digest),
            "{filename}: figure footer must carry the same digest the caption cites"
        );
    }
}

#[test]
fn negative_and_positive_findings_share_the_same_table_register() {
    let findings = small_dossier()["findings"].as_array().unwrap();
    let negative = findings
        .iter()
        .find(|f| f["negative"] == json!(true))
        .expect("the reference-like world yields at least one tie, a negative finding");
    let positive = findings
        .iter()
        .find(|f| f["negative"] == json!(false))
        .expect("the anchor finding is positive");
    let rendered = render_report(small_dossier()).expect("renders");
    let row_of = |finding: &Value| {
        let claim = finding["claim"].as_str().unwrap();
        rendered
            .report_md
            .lines()
            .find(|line| line.contains(claim))
            .unwrap_or_else(|| panic!("claim not rendered: {claim}"))
            .to_string()
    };
    let negative_row = row_of(negative);
    let positive_row = row_of(positive);
    assert!(negative_row.starts_with("| ") && positive_row.starts_with("| "));
    assert!(negative_row.contains("| negative observation |"));
    assert!(positive_row.contains("| observation |"));
}

#[test]
fn report_rendering_is_deterministic() {
    let first = render_report(full_dossier()).expect("renders");
    let second = render_report(full_dossier()).expect("renders");
    assert_eq!(first, second);
}

#[test]
fn a_digest_only_figure_source_is_refused_by_the_renderer() {
    let mut hollowed = small_dossier().clone();
    for step in hollowed["steps"].as_array_mut().unwrap() {
        for output in step["outputs"].as_array_mut().unwrap() {
            if output["name"] == json!("comparison-d40") {
                output.as_object_mut().unwrap().remove("artifact");
                output["inlined"] = json!(false);
            }
        }
    }
    match render_report(&hollowed) {
        Err(ResearchError::ArtifactNotInlined { name, .. }) => {
            assert_eq!(name, "comparison-d40");
        }
        other => panic!("must refuse rather than skip the figure, got {other:?}"),
    }
}

// ---------------------------------------------------------------- the full protocol

#[test]
fn a_full_protocol_run_carries_sweep_mutation_and_minimization_evidence() {
    let dossier = full_dossier();
    for name in ["sweep-table", "mutation-family", "mutation-diversity", "minimization"] {
        artifact(dossier, name);
    }
    let verification = verify_dossier(dossier).expect("verifiable");
    assert_eq!(verification["valid"], json!(true), "{verification}");

    let rules: Vec<&str> = dossier["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["rule"].as_str().unwrap())
        .collect();
    assert!(rules.contains(&"mutation_yield"));
    assert!(rules.contains(&"minimize_reduction"));
    assert!(
        rules.iter().any(|rule| rule.starts_with("sweep_")),
        "the sweep must derive at least one finding: {rules:?}"
    );
    let sweep = artifact(dossier, "sweep-table");
    assert_eq!(sweep["artifact"]["cells_total"], json!(36));
    assert!(sweep["artifact"]["caveat"]
        .as_str()
        .unwrap()
        .contains("deliberately not swept"));
}

#[test]
fn the_dossier_echoes_the_planned_protocol_and_every_step_completed() {
    let dossier = full_dossier();
    let planned = serde_json::to_value(plan_protocol(&full_request())).unwrap();
    assert_eq!(dossier["protocol"], planned);
    let steps = dossier["steps"].as_array().unwrap();
    assert_eq!(steps.len(), planned["steps"].as_array().unwrap().len());
    for (index, step) in steps.iter().enumerate() {
        assert_eq!(step["step_index"], json!(index));
        assert_eq!(step["outcome"], json!("completed"));
        assert_eq!(step["step"], planned["steps"][index]);
    }
}


// ---------------------------------------------------------------- the configuration matrix
//
// Every claim above reads one or two runs. These read a matrix that varies the four world
// families, six flag patterns, ascending and descending point sets, and paired seeds — the
// axes along which a single-configuration test is blind.

/// One configuration of the depth matrix.
struct MatrixEntry {
    research_id: &'static str,
    family: &'static str,
    points: &'static [u32],
    seed: u64,
    sweep: bool,
    mutation: bool,
    minimize: bool,
}

const fn entry(
    research_id: &'static str,
    family: &'static str,
    points: &'static [u32],
    seed: u64,
    sweep: bool,
    mutation: bool,
    minimize: bool,
) -> MatrixEntry {
    MatrixEntry {
        research_id,
        family,
        points,
        seed,
        sweep,
        mutation,
        minimize,
    }
}

/// The matrix.
///
/// Exactly one entry runs the sweep: the sweep executes the committed default grid at the
/// grid's own seed, so it is identical work in every request that asks for it, and a second
/// sweeping entry would buy coverage of nothing but its own runtime.
#[rustfmt::skip]
const DEPTH_MATRIX: [MatrixEntry; 8] = [
    entry("depth-ref-bare",        "reference_like",        &[0],                1,             false, false, false),
    entry("depth-disc-multi",      "discriminating",        &[20, 60],           20_260_823,    false, false, false),
    entry("depth-disc-reseeded",   "discriminating",        &[20, 60],           20_260_824,    false, false, false),
    entry("depth-extconf-mutate",  "external_confirmation", &[30],               7,             false, true,  false),
    entry("depth-policy-minimize", "policy_restricted",     &[15],               u64::MAX,      false, false, true),
    entry("depth-ref-descending",  "reference_like",        &[45, 5],            4_294_967_296, false, true,  true),
    entry("depth-disc-full",       "discriminating",        &[25, 75],           0,             true,  true,  true),
    entry("depth-policy-six",      "policy_restricted",     &[1, 2, 3, 4, 5, 6], 123_456_789,   false, true,  true),
];

fn matrix_request(index: usize) -> ResearchRequest {
    let entry = &DEPTH_MATRIX[index];
    request_of(json!({
        "research_id": entry.research_id,
        "question": "Does this configuration reproduce, cite, and account for itself?",
        "family": entry.family,
        "distractor_points": entry.points,
        "seed": entry.seed,
        "run_sweep": entry.sweep,
        "run_mutation": entry.mutation,
        "run_minimize": entry.minimize,
    }))
}

fn matrix_dossiers() -> &'static Vec<Value> {
    static DOSSIERS: OnceLock<Vec<Value>> = OnceLock::new();
    DOSSIERS.get_or_init(|| {
        (0..DEPTH_MATRIX.len())
            .map(|index| {
                run_research(&matrix_request(index)).unwrap_or_else(|error| {
                    panic!("{} must run: {error}", DEPTH_MATRIX[index].research_id)
                })
            })
            .collect()
    })
}

fn recorded_artifacts(dossier: &Value) -> Vec<(&str, &str, Option<&Value>)> {
    dossier["steps"]
        .as_array()
        .expect("steps array")
        .iter()
        .flat_map(|step| step["outputs"].as_array().expect("outputs array"))
        .map(|output| {
            (
                output["name"].as_str().expect("artifact name"),
                output["sha256"].as_str().expect("artifact digest"),
                output.get("artifact"),
            )
        })
        .collect()
}

#[test]
fn every_configuration_in_the_matrix_reproduces_its_dossier_report_and_figures_byte_for_byte() {
    for (index, entry) in DEPTH_MATRIX.iter().enumerate() {
        let id = entry.research_id;
        let first = &matrix_dossiers()[index];
        let second = run_research(&matrix_request(index)).expect("second run completes");
        assert_eq!(
            to_canonical_string(first).unwrap(),
            to_canonical_string(&second).unwrap(),
            "{id}: the dossier must be a deterministic function of the request"
        );
        let rendered_first = render_report(first).expect("first render");
        let rendered_second = render_report(&second).expect("second render");
        assert_eq!(
            rendered_first.report_md, rendered_second.report_md,
            "{id}: the report must render identically from an identical dossier"
        );
        assert_eq!(
            rendered_first.figures, rendered_second.figures,
            "{id}: every figure must render identically from an identical dossier"
        );
    }
}

#[test]
fn the_reference_anchor_finding_is_identical_across_every_family_seed_and_point_set() {
    let mut anchors: Vec<(&str, &Value, &str)> = Vec::new();
    for (index, entry) in DEPTH_MATRIX.iter().enumerate() {
        let id = entry.research_id;
        let dossier = &matrix_dossiers()[index];
        let findings = dossier["findings"].as_array().expect("findings array");
        let anchor = findings
            .iter()
            .find(|finding| finding["rule"] == json!("reference_anchor"))
            .unwrap_or_else(|| panic!("{id}: every dossier anchors to the pinned certificate"));
        assert_eq!(
            findings
                .iter()
                .filter(|finding| finding["rule"] == json!("reference_anchor"))
                .count(),
            1,
            "{id}: the anchor is the run's trust root and is recorded exactly once"
        );
        let certificate = recorded_artifacts(dossier)
            .into_iter()
            .find(|(name, ..)| *name == "reference-certificate")
            .unwrap_or_else(|| panic!("{id}: the anchor step records its certificate"));
        assert_eq!(
            certificate.2.expect("the certificate always inlines")["certificate_sha256"],
            json!(PINNED_REFERENCE_CERTIFICATE_SHA256),
            "{id}: the recorded certificate carries the pinned parity digest"
        );
        anchors.push((id, anchor, certificate.1));
    }
    let (first_id, first_anchor, first_digest) = anchors[0];
    for (id, anchor, digest) in &anchors[1..] {
        assert_eq!(
            to_canonical_string(anchor).unwrap(),
            to_canonical_string(first_anchor).unwrap(),
            "{id} and {first_id} must record the same anchor: the committed fixture pair does \
             not depend on the request's family, seed or points"
        );
        assert_eq!(
            *digest, first_digest,
            "{id} and {first_id} must cite the same certificate artifact digest"
        );
    }
}

#[test]
fn every_findings_support_digest_names_an_artifact_its_own_dossier_carries() {
    let mut checked = 0usize;
    for (index, entry) in DEPTH_MATRIX.iter().enumerate() {
        let id = entry.research_id;
        let dossier = &matrix_dossiers()[index];
        let known: Vec<&str> = recorded_artifacts(dossier)
            .into_iter()
            .map(|(_, digest, _)| digest)
            .collect();
        for finding in dossier["findings"].as_array().expect("findings array") {
            let supported = finding["supported_by"]
                .as_array()
                .expect("supported_by array");
            assert!(
                !supported.is_empty(),
                "{id}: finding {} cites nothing",
                finding["rule"]
            );
            for digest in supported {
                let digest = digest.as_str().expect("citation is a string");
                assert!(
                    known.contains(&digest),
                    "{id}: finding {} cites {digest}, which no recorded artifact carries",
                    finding["rule"]
                );
                checked += 1;
            }
        }
        let verification = verify_dossier(dossier).expect("verifiable");
        assert_eq!(verification["valid"], json!(true), "{id}: {verification}");
    }
    assert!(
        checked >= DEPTH_MATRIX.len() * 3,
        "the matrix must exercise more than a handful of citations, checked {checked}"
    );
}

#[test]
fn every_figure_footer_digest_equals_the_dossier_record_of_the_artifact_its_caption_names() {
    let mut checked = 0usize;
    for (index, entry) in DEPTH_MATRIX.iter().enumerate() {
        let id = entry.research_id;
        let dossier = &matrix_dossiers()[index];
        let rendered = render_report(dossier).expect("renders");
        let artifacts = recorded_artifacts(dossier);
        for (filename, svg) in &rendered.figures {
            let caption_start = rendered
                .report_md
                .find(filename.as_str())
                .unwrap_or_else(|| panic!("{id}: {filename} must be captioned"));
            let caption = &rendered.report_md[caption_start..];
            let source_line = caption
                .lines()
                .find(|line| line.starts_with("Source artifact `"))
                .unwrap_or_else(|| panic!("{id}: {filename} must name its source artifact"));
            let artifact_name = source_line
                .split("Source artifact `")
                .nth(1)
                .and_then(|rest| rest.split('`').next())
                .expect("artifact name extractable");
            let caption_digest = source_line
                .split("sha256 `")
                .nth(1)
                .and_then(|rest| rest.split('`').next())
                .expect("caption digest extractable");
            let recorded = artifacts
                .iter()
                .find(|(name, ..)| *name == artifact_name)
                .unwrap_or_else(|| {
                    panic!(
                        "{id}: {filename} names {artifact_name}, which the dossier does not carry"
                    )
                });
            assert_eq!(
                recorded.1, caption_digest,
                "{id}: {filename}'s caption digest must be the dossier's record for \
                 {artifact_name}"
            );
            let footer = format!("source sha256: {caption_digest}");
            assert!(
                svg.contains(&footer),
                "{id}: {filename}'s footer must carry the digest its caption cites"
            );
            let rendered_value = recorded.2.unwrap_or_else(|| {
                panic!(
                    "{id}: a figure source must inline, or the figure cites what no reader can \
                     check"
                )
            });
            assert_eq!(
                ContentHash::of_value(rendered_value).unwrap().to_string(),
                caption_digest,
                "{id}: {filename}'s digest must recompute from the artifact the dossier carries"
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 20,
        "the matrix must exercise every figure kind more than once, checked {checked}"
    );
}

#[test]
fn the_sweep_findings_counts_recompute_from_the_sweep_table_the_dossier_carries() {
    let index = DEPTH_MATRIX
        .iter()
        .position(|entry| entry.sweep)
        .expect("the matrix runs the sweep somewhere");
    let dossier = &matrix_dossiers()[index];
    let artifacts = recorded_artifacts(dossier);
    let table = artifacts
        .iter()
        .find(|(name, ..)| *name == "sweep-table")
        .expect("the sweep step records its table")
        .2
        .expect("the sweep table always inlines");
    let cells = table["cells"].as_array().expect("cells array");
    let total = cells.len();
    let (mut ties, mut fiber_only, mut fiber_inadmissible, mut none_admissible) = (0, 0, 0, 0);
    for cell in cells {
        let rows = cell["rows"].as_array().expect("rows array");
        let fiber_admissible = rows
            .iter()
            .find(|row| row["strategy"] == json!("fiber"))
            .is_some_and(|row| row["admissible"] == json!(true));
        let baselines = rows
            .iter()
            .filter(|row| {
                row["strategy"] != json!("fiber") && row["strategy"] != json!("full-context")
            })
            .filter(|row| row["admissible"] == json!(true))
            .count();
        match (fiber_admissible, baselines) {
            (true, 0) => fiber_only += 1,
            (true, _) => ties += 1,
            (false, 0) => {
                fiber_inadmissible += 1;
                none_admissible += 1;
            }
            (false, _) => fiber_inadmissible += 1,
        }
    }
    let claims: Vec<(&str, &str)> = dossier["findings"]
        .as_array()
        .expect("findings array")
        .iter()
        .map(|finding| {
            (
                finding["rule"].as_str().expect("rule"),
                finding["claim"].as_str().expect("claim"),
            )
        })
        .collect();
    for (rule, count) in [
        ("sweep_ties", ties),
        ("sweep_fiber_only", fiber_only),
        ("sweep_fiber_inadmissible", fiber_inadmissible),
        ("sweep_none_admissible", none_admissible),
    ] {
        let claim = claims
            .iter()
            .find(|(name, _)| *name == rule)
            .map(|(_, claim)| *claim);
        match (count, claim) {
            (0, Some(claim)) => panic!("{rule} claims {claim:?} but the table has no such cell"),
            (0, None) => {}
            (count, Some(claim)) => assert!(
                claim.contains(&format!("{count} of {total}")),
                "{rule} must state {count} of {total}, claims {claim:?}"
            ),
            (count, None) => panic!("{count} of {total} cells are {rule} but no finding says so"),
        }
    }
}

#[test]
fn the_mutation_and_minimization_steps_target_the_first_declared_point_not_the_smallest() {
    let index = DEPTH_MATRIX
        .iter()
        .position(|entry| entry.research_id == "depth-ref-descending")
        .expect("the matrix declares a descending point set");
    let points = DEPTH_MATRIX[index].points;
    assert!(
        points[0] > points[points.len() - 1],
        "this claim is only meaningful over points that descend"
    );
    let dossier = &matrix_dossiers()[index];
    for kind in ["mutate_base_world", "minimize_base_world"] {
        let step = dossier["steps"]
            .as_array()
            .expect("steps array")
            .iter()
            .find(|step| step["step"]["kind"] == json!(kind))
            .unwrap_or_else(|| panic!("the run must carry a {kind} step"));
        assert_eq!(
            step["step"]["distractors"],
            json!(points[0]),
            "{kind} must target the first declared point, not the smallest one"
        );
    }
    let rendered = render_report(dossier).expect("renders");
    assert!(
        rendered.report_md.contains(&format!(
            "bioprism mutate family --world world-d{}.json",
            points[0]
        )),
        "the reproduction section must name the same base world the mutation step used"
    );
    assert!(
        rendered.report_md.contains(&format!(
            "bioprism prism minimize --world world-d{}.json",
            points[0]
        )),
        "the reproduction section must name the same base world the minimization step used"
    );
    let listed = rendered
        .report_md
        .lines()
        .find(|line| line.starts_with("- distractor points: "))
        .expect("the report lists its points");
    assert_eq!(
        listed,
        format!(
            "- distractor points: {}",
            points
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        "the report must list the points in the order the request declared them"
    );
}
