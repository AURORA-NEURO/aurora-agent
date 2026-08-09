//! The shipped oracles, the provider contract, and compatibility with `bioprism_section`.

mod common;

use std::collections::BTreeSet;

use bioprism_oracle::{
    Confidence, Evidence, EvidenceTier, FieldType, Finding, Judgement, MeshPolicy, MockJudgeOracle,
    NumericProperty, Oracle, OracleError, OracleId, OracleManifest, OracleMesh, OracleRef,
    OracleVersion, Plane, Position, PositionDistribution, PropertyOracle, Recheck,
    ReexecutionOracle, SchemaOracle, WorldDocumentOracle,
};
use bioprism_section::{LeakageWitness, OracleStatus, OracleVerdict};
use serde_json::json;

use common::{always, evidence, now, oracle_ref};

fn version() -> OracleVersion {
    OracleVersion::new(1, 0, 0)
}

fn schema() -> SchemaOracle {
    SchemaOracle::new("bioprism:survival_schema", version(), always())
        .expect("the fixture oracle declares a plane")
        .require("index_date", FieldType::String)
        .require("event_date", FieldType::String)
        .require("months", FieldType::Number)
}

#[test]
fn a_schema_oracle_contradicts_a_missing_required_field_without_consulting_a_judge() {
    let judged = schema()
        .evaluate(
            &evidence("bundle")
                .with_field("index_date", json!("2026-01-01T00:00:00Z"))
                .with_field("months", json!(12.0)),
        )
        .expect("a well-formed oracle does not fail on a defective artifact");

    assert_eq!(judged.position, Position::Contradicted);
    assert_eq!(judged.tier, EvidenceTier::Deterministic);
    assert!(judged.findings.iter().any(|finding| matches!(
        finding,
        Finding::MissingField { pointer } if pointer == "event_date"
    )));
    assert!(judged.is_fully_checkable());
}

#[test]
fn a_schema_oracle_reports_a_type_mismatch_rather_than_coercing_the_value() {
    let judged = schema()
        .evaluate(
            &evidence("bundle")
                .with_field("index_date", json!("2026-01-01T00:00:00Z"))
                .with_field("event_date", json!("2026-06-01T00:00:00Z"))
                .with_field("months", json!("12")),
        )
        .expect("evaluation succeeds");

    assert_eq!(judged.position, Position::Contradicted);
    assert!(judged.findings.iter().any(|finding| matches!(
        finding,
        Finding::TypeMismatch { pointer, .. } if pointer == "months"
    )));
}

#[test]
fn a_schema_oracle_abstains_when_handed_an_unrelated_artifact() {
    let judged = schema()
        .evaluate(&evidence("hypothesis").with_field("prose", json!("PTEN loss drives growth")))
        .expect("evaluation succeeds");

    assert_eq!(
        judged.position,
        Position::NotEvaluable,
        "being handed the wrong artifact is not the same as being handed a broken one"
    );
    assert!(judged.findings.is_empty());
}

#[test]
fn a_checksum_over_canonical_bytes_survives_key_reordering() {
    let oracle = SchemaOracle::new("bioprism:checksum", version(), always())
        .expect("the fixture oracle declares a plane")
        .with_checksum("payload_sha256", "payload");

    let hashed = bioprism_ids::sha256_hex_of_value(&json!({"a": 1, "b": 2}))
        .expect("the payload is canonically serializable");

    let reordered = oracle
        .evaluate(
            &evidence("bundle")
                .with_field("payload", json!({"b": 2, "a": 1}))
                .with_field("payload_sha256", json!(hashed.clone())),
        )
        .expect("evaluation succeeds");
    assert_eq!(
        reordered.position,
        Position::Supported,
        "canonicalising first means the check reports payload changes, not serializer changes"
    );

    let tampered = oracle
        .evaluate(
            &evidence("bundle")
                .with_field("payload", json!({"a": 1, "b": 3}))
                .with_field("payload_sha256", json!(hashed)),
        )
        .expect("evaluation succeeds");
    assert_eq!(tampered.position, Position::Contradicted);
    assert!(tampered
        .findings
        .iter()
        .any(|finding| matches!(finding, Finding::ChecksumMismatch { .. })));
}

#[test]
fn an_event_date_preceding_the_index_date_is_contradicted_with_no_model_in_the_loop() {
    let oracle = PropertyOracle::new("bioprism:survival_properties", version(), always())
        .expect("the fixture oracle declares a plane")
        .check(NumericProperty::OrderedInstants {
            earlier: "index_date".to_string(),
            later: "event_date".to_string(),
        });

    let judged = oracle
        .evaluate(
            &evidence("bundle")
                .with_field("index_date", json!("2026-06-01T00:00:00Z"))
                .with_field("event_date", json!("2026-01-01T00:00:00Z")),
        )
        .expect("evaluation succeeds");

    assert_eq!(judged.position, Position::Contradicted);
    assert_eq!(judged.tier, EvidenceTier::Property);
    assert!(judged
        .findings
        .iter()
        .any(|finding| matches!(finding, Finding::PropertyViolated { .. })));
}

#[test]
fn an_ordering_property_over_a_non_timestamp_is_a_configuration_error_not_a_contradiction() {
    let oracle = PropertyOracle::new("bioprism:survival_properties", version(), always())
        .expect("the fixture oracle declares a plane")
        .check(NumericProperty::OrderedInstants {
            earlier: "index_date".to_string(),
            later: "event_date".to_string(),
        });

    let error = oracle
        .evaluate(
            &evidence("bundle")
                .with_field("index_date", json!("day one"))
                .with_field("event_date", json!("2026-01-01T00:00:00Z")),
        )
        .expect_err("an uncomparable field is a fault of the oracle, not of the artifact");

    assert!(matches!(error, OracleError::NonComparableField { .. }));
}

#[test]
fn a_property_suite_whose_checks_all_skip_abstains_rather_than_passing() {
    let oracle = PropertyOracle::new("bioprism:survival_properties", version(), always())
        .expect("the fixture oracle declares a plane")
        .check(NumericProperty::Bounded {
            field: "fraction".to_string(),
            low: 0.0,
            high: 1.0,
        });

    let judged = oracle
        .evaluate(&evidence("bundle").with_field("unrelated", json!(7)))
        .expect("evaluation succeeds");

    assert_eq!(
        judged.position,
        Position::NotEvaluable,
        "a pass earned by running no checks is the commonest way a property suite dies quietly"
    );
    assert!(judged
        .findings
        .iter()
        .any(|finding| matches!(finding, Finding::NotApplicable { .. })));
}

#[test]
fn a_conservation_property_detects_parts_that_do_not_sum_to_their_total() {
    let oracle = PropertyOracle::new("bioprism:dose_volume", version(), always())
        .expect("the fixture oracle declares a plane")
        .check(NumericProperty::ConservesTotal {
            parts: vec!["left".to_string(), "right".to_string()],
            total: "whole".to_string(),
            tolerance: 1e-6,
        })
        .check(NumericProperty::NonDecreasing {
            series: "cumulative".to_string(),
        });

    let judged = oracle
        .evaluate(
            &evidence("bundle")
                .with_field("left", json!(1.0))
                .with_field("right", json!(2.0))
                .with_field("whole", json!(4.0))
                .with_field("cumulative", json!([0.0, 1.0, 0.5])),
        )
        .expect("evaluation succeeds");

    assert_eq!(judged.position, Position::Contradicted);
    assert_eq!(
        judged.findings.len(),
        2,
        "both violated properties are reported, not just the first"
    );
}

#[test]
fn a_reported_interval_excluding_the_recomputed_estimate_fails_result_consistency() {
    let oracle = ReexecutionOracle::new("bioprism:rerun", version(), always())
        .expect("the fixture oracle declares a plane")
        .check(Recheck::IntervalCovers {
            estimate: "recomputed_estimate".to_string(),
            low: "reported_ci_low".to_string(),
            high: "reported_ci_high".to_string(),
        });

    let judged = oracle
        .evaluate(
            &evidence("notebook")
                .with_field("recomputed_estimate", json!(0.91))
                .with_field("reported_ci_low", json!(0.40))
                .with_field("reported_ci_high", json!(0.72)),
        )
        .expect("evaluation succeeds");

    assert_eq!(judged.position, Position::Contradicted);
    assert_eq!(
        judged.tier,
        EvidenceTier::Execution,
        "the comparison is exact, but its inputs came from a run this crate did not witness"
    );
}

#[test]
fn a_numeric_recheck_inside_tolerance_supports_the_report() {
    let oracle = ReexecutionOracle::new("bioprism:rerun", version(), always())
        .expect("the fixture oracle declares a plane")
        .check(Recheck::Numeric {
            reported: "reported_auc".to_string(),
            recomputed: "recomputed_auc".to_string(),
            tolerance: 1e-3,
        });

    let judged = oracle
        .evaluate(
            &evidence("notebook")
                .with_field("reported_auc", json!(0.8412))
                .with_field("recomputed_auc", json!(0.8415)),
        )
        .expect("evaluation succeeds");

    assert_eq!(judged.position, Position::Supported);
}

#[test]
fn a_judge_finding_is_not_checkable_while_a_deterministic_finding_is() {
    let judge = MockJudgeOracle::new(
        "bioprism:plan_review",
        version(),
        always(),
        "does the plan acknowledge confounding?",
    )
    .expect("the fixture oracle declares a plane")
    .scripting(
        "bundle",
        Position::Supported,
        Confidence::new(0.95).expect("a probability"),
        "the plan names two confounders and a control strategy",
    );

    let opinion = judge
        .evaluate(&evidence("bundle"))
        .expect("evaluation succeeds");
    assert!(!opinion.is_fully_checkable());
    assert!(opinion
        .findings
        .iter()
        .all(|finding| matches!(finding, Finding::Remark { .. })));

    let checked = schema()
        .evaluate(&evidence("bundle").with_field("index_date", json!("2026-01-01T00:00:00Z")))
        .expect("evaluation succeeds");
    assert!(checked.is_fully_checkable());
}

#[test]
fn an_unscripted_judge_abstains_rather_than_guessing() {
    let judge = MockJudgeOracle::new("bioprism:plan_review", version(), always(), "rubric")
        .expect("the fixture oracle declares a plane");

    let opinion = judge
        .evaluate(&evidence("never-seen"))
        .expect("evaluation succeeds");
    assert_eq!(opinion.position, Position::NotEvaluable);
}

#[test]
fn every_shipped_oracle_disclaims_the_biological_plane() {
    let judge = MockJudgeOracle::new("bioprism:plan_review", version(), always(), "rubric")
        .expect("the fixture oracle declares a plane");
    let rerun = ReexecutionOracle::new("bioprism:rerun", version(), always())
        .expect("the fixture oracle declares a plane");
    let properties = PropertyOracle::new("bioprism:props", version(), always())
        .expect("the fixture oracle declares a plane");
    let world = WorldDocumentOracle::new("bioprism:world_doc", version(), always())
        .expect("the fixture oracle declares a plane");

    let oracles: [&dyn Oracle; 5] = [&schema(), &judge, &rerun, &properties, &world];
    for oracle in oracles {
        assert!(
            oracle
                .manifest()
                .cannot_establish
                .contains(&Plane::Biological),
            "{} must say out loud that it establishes nothing biological",
            oracle.kind()
        );
    }
}

#[test]
fn a_malformed_world_document_is_contradicted_by_the_deterministic_parser_oracle() {
    let oracle = WorldDocumentOracle::new("bioprism:world_doc", version(), always())
        .expect("the fixture oracle declares a plane");

    let judged = oracle
        .evaluate(&evidence("world-bundle").with_field("world", json!({"world": {}})))
        .expect("evaluation succeeds");

    assert_eq!(judged.position, Position::Contradicted);
    assert_eq!(judged.tier, EvidenceTier::Deterministic);
    assert!(judged
        .findings
        .iter()
        .any(|finding| matches!(finding, Finding::Malformed { .. })));

    let absent = oracle
        .evaluate(&evidence("not-a-world"))
        .expect("evaluation succeeds");
    assert_eq!(absent.position, Position::NotEvaluable);
}

#[test]
fn an_oracle_that_claims_a_plane_it_also_disclaims_is_rejected() {
    let error = OracleManifest::new(
        oracle_ref("contradictory", 1),
        EvidenceTier::Deterministic,
        [Plane::Artifact, Plane::Policy],
        [Plane::Policy],
        always(),
    )
    .expect_err("40.21 invariant 1 forbids saying both things about one plane");

    assert!(matches!(
        error,
        OracleError::ContradictoryPlaneDeclaration {
            plane: Plane::Policy,
            ..
        }
    ));
}

#[test]
fn an_oracle_that_establishes_nothing_is_rejected() {
    let error = OracleManifest::new(
        oracle_ref("mute", 1),
        EvidenceTier::Deterministic,
        [],
        [Plane::Artifact],
        always(),
    )
    .expect_err("an oracle establishing nothing has no reason to run");

    assert!(matches!(error, OracleError::NoEstablishedPlane { .. }));
}

#[test]
fn an_oracle_id_without_a_namespace_is_rejected() {
    assert!(OracleId::parse("schema").is_err());
    assert!(OracleId::parse("bioprism:").is_err());
    assert!(OracleId::parse("a:b:c").is_err());
    assert_eq!(
        OracleRef::new(
            OracleId::parse("bioprism:schema").expect("well formed"),
            OracleVersion::new(2, 1, 0),
        )
        .to_string(),
        "biooracle:bioprism:schema:2.1.0"
    );
}

#[test]
fn a_mesh_rejects_the_same_oracle_twice_so_no_position_is_counted_twice() {
    let mut mesh = OracleMesh::new(MeshPolicy::default());
    mesh.register(Box::new(schema()))
        .expect("the first registration succeeds");

    let error = mesh
        .register(Box::new(schema()))
        .expect_err("a duplicate would be a vote arrived at by accident");
    assert!(matches!(error, OracleError::DuplicateOracle { .. }));
    assert_eq!(mesh.len(), 1);
}

#[test]
fn an_empty_mesh_refuses_to_produce_a_verdict() {
    let mesh = OracleMesh::new(MeshPolicy::default());
    assert!(mesh.is_empty());
    assert_eq!(
        mesh.evaluate(&evidence("bundle")),
        Err(OracleError::EmptyMesh)
    );
}

#[test]
fn a_mesh_runs_every_oracle_and_the_ladder_decides_the_verdict() {
    let judge = MockJudgeOracle::new(
        "bioprism:plan_review",
        version(),
        always(),
        "is the bundle plausible?",
    )
    .expect("the fixture oracle declares a plane")
    .scripting(
        "bundle",
        Position::Supported,
        Confidence::new(0.99).expect("a probability"),
        "reads fine to me",
    );

    let mesh = OracleMesh::new(MeshPolicy::default())
        .with(Box::new(schema()))
        .expect("registration succeeds")
        .with(Box::new(judge))
        .expect("registration succeeds");

    let verdict = mesh
        .evaluate(
            &evidence("bundle")
                .with_field("index_date", json!("2026-01-01T00:00:00Z"))
                .with_field("months", json!(12.0)),
        )
        .expect("a non-empty mesh produces a verdict");

    assert_eq!(verdict.status(), OracleStatus::Invalid);
    assert_eq!(verdict.suppressed.len(), 1);
    assert!(verdict.failures.is_empty());
    assert!(!verdict.is_judge_only());
}

#[test]
fn a_failing_oracle_is_reported_without_taking_the_rest_of_the_mesh_down() {
    let broken = PropertyOracle::new("bioprism:broken", version(), always())
        .expect("the fixture oracle declares a plane")
        .check(NumericProperty::OrderedInstants {
            earlier: "a".to_string(),
            later: "b".to_string(),
        });

    let mesh = OracleMesh::new(MeshPolicy::default())
        .with(Box::new(schema()))
        .expect("registration succeeds")
        .with(Box::new(broken))
        .expect("registration succeeds");

    let verdict = mesh
        .evaluate(
            &evidence("bundle")
                .with_field("index_date", json!("2026-01-01T00:00:00Z"))
                .with_field("event_date", json!("2026-06-01T00:00:00Z"))
                .with_field("months", json!(12.0))
                .with_field("a", json!("not a timestamp"))
                .with_field("b", json!("2026-01-01T00:00:00Z")),
        )
        .expect("a broken grader does not abort the mesh");

    assert_eq!(verdict.status(), OracleStatus::Valid);
    assert_eq!(verdict.failures.len(), 1);
    assert_eq!(verdict.contributing.len(), 1);
}

#[test]
fn a_fiber_split_integrity_verdict_lifts_onto_the_ladder_and_projects_back() {
    let manifest = OracleManifest::new(
        OracleRef::new(
            OracleId::parse("fiber:split_integrity").expect("well formed"),
            version(),
        ),
        EvidenceTier::Deterministic,
        [Plane::Artifact],
        [],
        always(),
    )
    .expect("the manifest declares a plane")
    .disclaiming_the_rest();

    let original = OracleVerdict::new(
        "deterministic_split_integrity_v1",
        vec![LeakageWitness::IdentityLeakage {
            alias: "ALT-77".to_string(),
            subjects: vec!["S001".to_string(), "S003".to_string()],
            splits: vec!["train".to_string(), "test".to_string()],
        }],
    );

    let lifted = Judgement::lift_verdict(&manifest, &now(), &original);
    assert_eq!(lifted.position, Position::Contradicted);
    assert_eq!(lifted.tier, EvidenceTier::Deterministic);
    assert_eq!(lifted.confidence, Confidence::CERTAIN);
    assert_eq!(lifted.findings.len(), 1);

    let projected = lifted.to_verdict();
    assert_eq!(projected.status, original.status);
    assert_eq!(projected.witnesses, original.witnesses);
    assert_eq!(projected.oracle_kind, "fiber:split_integrity");
}

#[test]
fn projecting_a_judgement_onto_the_older_verdict_shape_drops_uncheckable_findings() {
    let judge = MockJudgeOracle::new("bioprism:plan_review", version(), always(), "rubric")
        .expect("the fixture oracle declares a plane")
        .scripting(
            "bundle",
            Position::Contradicted,
            Confidence::new(0.9).expect("a probability"),
            "the plan ignores batch effects",
        );

    let projected = judge
        .evaluate(&evidence("bundle"))
        .expect("evaluation succeeds")
        .to_verdict();

    assert_eq!(projected.status, OracleStatus::Invalid);
    assert!(
        projected.witnesses.is_empty(),
        "a remark has no witness shape, and inventing one would put a falsehood in a certificate"
    );
}

#[test]
fn a_distribution_returns_tied_modes_as_a_set_rather_than_choosing_one() {
    let tied =
        PositionDistribution::new([(Position::Supported, 0.5), (Position::Contradicted, 0.5)])
            .expect("the mass sums to one");

    assert_eq!(
        tied.modes(),
        BTreeSet::from([Position::Supported, Position::Contradicted]),
        "picking the first of two equal modes is consensus collapse performed silently"
    );

    let spread = PositionDistribution::new([
        (Position::Supported, 0.55),
        (Position::Contradicted, 0.35),
        (Position::Unresolved, 0.10),
    ])
    .expect("the mass sums to one");
    assert_eq!(spread.modes(), BTreeSet::from([Position::Supported]));
    assert_eq!(spread.mass(Position::NotEvaluable), 0.0);
}

#[test]
fn a_distribution_whose_mass_does_not_sum_to_one_is_rejected() {
    assert!(matches!(
        PositionDistribution::new([(Position::Supported, 0.5)])
            .expect_err("half a distribution is not a distribution"),
        OracleError::MalformedDistribution { .. }
    ));
    assert!(PositionDistribution::new([
        (Position::Supported, 1.5),
        (Position::Contradicted, -0.5),
    ])
    .is_err());
    assert!(PositionDistribution::new([]).is_err());
}

#[test]
fn confidence_outside_the_unit_interval_is_rejected() {
    assert!(Confidence::new(-0.1).is_err());
    assert!(Confidence::new(1.1).is_err());
    assert!(Confidence::new(f64::NAN).is_err());
    assert_eq!(Confidence::CERTAIN.value(), 1.0);
}

#[test]
fn a_judgement_round_trips_through_json_with_its_tier_and_admissibility_intact() {
    let judged = schema()
        .evaluate(&evidence("bundle").with_field("index_date", json!("2026-01-01T00:00:00Z")))
        .expect("evaluation succeeds");

    let encoded = serde_json::to_string(&judged).expect("a judgement is serializable");
    let decoded: Judgement = serde_json::from_str(&encoded).expect("and deserializable");
    assert_eq!(decoded, judged);
}

#[test]
fn the_provider_contract_reports_the_effective_tier_not_the_declared_one() {
    let mut oracle = schema();
    oracle
        .manifest_mut()
        .independence
        .shared
        .insert(bioprism_oracle::SharedResource::PreprocessingCode);

    assert_eq!(oracle.manifest().declared_tier, EvidenceTier::Deterministic);
    assert_eq!(
        oracle.tier(),
        EvidenceTier::Execution,
        "the optimistic number is harder to reach for than the honest one"
    );
    assert_eq!(oracle.kind(), "bioprism:survival_schema");
}

#[test]
fn evidence_carries_the_evaluation_instant_rather_than_reading_a_clock() {
    let earlier = Evidence::new("bundle", now());
    let later = Evidence::new(
        "bundle",
        bioprism_oracle::UtcTimestamp::parse("2027-01-01T00:00:00Z").expect("well formed"),
    );

    assert!(earlier.at < later.at);
    assert!(earlier.require_field("absent").is_err());
}
